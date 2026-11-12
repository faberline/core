use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tempfile::TempDir;
use tokio::task::JoinHandle;

use raft_runtime::{
    FsyncPolicy, HostConfig, Index, Membership, NodeId, RaftHost, RaftStateMachine, RaftStore,
    ReadIndexError,
};

use crate::support::cluster;
use cluster::*;

fn read_cfg() -> HostConfig {
    HostConfig::default().with_propose_timeout(Duration::from_millis(600))
}

#[tokio::test]
async fn leader_returns_commit_index_after_quorum_round() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");

    nodes[leader]
        .host
        .propose(vec![1])
        .await
        .expect("first proposal commits");
    nodes[leader]
        .host
        .propose(vec![2])
        .await
        .expect("second proposal commits");

    let idx = nodes[leader]
        .host
        .read_index()
        .await
        .expect("leader grants a read index");
    assert!(idx >= Index::new(2), "read index {idx:?} is below 2");
}

#[tokio::test]
async fn read_index_appends_no_log_entry() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");

    nodes[leader].host.propose(vec![1]).await.unwrap();
    nodes[leader].host.propose(vec![2]).await.unwrap();

    let before = nodes[leader].host.status().await.last_index;
    for _ in 0..5 {
        nodes[leader]
            .host
            .read_index()
            .await
            .expect("leader grants a read index");
    }
    let after = nodes[leader].host.status().await.last_index;
    assert_eq!(before, after, "read_index appended a log entry");
}

#[tokio::test]
async fn follower_reports_not_leader() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");
    let follower = (0..nodes.len())
        .find(|&i| i != leader)
        .expect("a follower exists");

    let res = nodes[follower].host.read_index().await;
    assert_eq!(res, Err(ReadIndexError::NotLeader));
}

#[tokio::test]
async fn fresh_leader_without_own_term_commit_refuses_or_grants() {
    let cfg = read_cfg();
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");

    // A no-op from the new term may already have committed, so only the
    // refusal or a grant is allowed here. Any other error is a bug.
    match nodes[leader].host.read_index().await {
        Ok(_) | Err(ReadIndexError::NoCommitInTerm) => {}
        other => panic!("unexpected read_index result: {other:?}"),
    }
}

/// Connection tasks accepted by one node's listener.
type Conns = Arc<StdMutex<Vec<JoinHandle<()>>>>;

/// Like `cluster_with_config`, but it keeps the accepted connection tasks so a
/// test can cut them. `cluster_with_config` detaches them, and after
/// `_serve.abort()` the old connections still reach the node's router, so the
/// leader keeps receiving acks from a "partitioned" follower.
async fn cuttable_cluster(n: u64, cfg: HostConfig) -> (Vec<Node>, Vec<Conns>) {
    let mut listeners = Vec::new();
    let mut all = Vec::new();
    for id in 0..n {
        let (l, url) = bind().await;
        listeners.push(l);
        all.push((NodeId::new(id), url));
    }
    let voters: Vec<NodeId> = (0..n).map(NodeId::new).collect();
    let mut nodes = Vec::new();
    let mut conns_by_node = Vec::new();
    for (idx, listener) in listeners.into_iter().enumerate() {
        let id = NodeId::new(idx as u64);
        let peers = peers_excluding(id, &all);
        let sm = TestSm::new();
        let dir = TempDir::new().unwrap();
        let store = RaftStore::open(dir.path().to_str().unwrap(), id, FsyncPolicy::Os).unwrap();
        let host = Arc::new(RaftHost::spawn(
            id,
            Membership::new(voters.clone(), vec![]),
            peers,
            store,
            sm.clone() as Arc<dyn RaftStateMachine>,
            cfg,
        ));
        let router = host.router();
        let conns: Conns = Arc::new(StdMutex::new(Vec::new()));
        let conns_serve = conns.clone();
        let serve = tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let r = router.clone();
                    let conn = tokio::spawn(async move {
                        let _ = transport_h2c::server::serve_connection(stream, r).await;
                    });
                    conns_serve.lock().unwrap().push(conn);
                }
            }
        });
        nodes.push(Node {
            host,
            sm,
            url: all[idx].1.clone(),
            _serve: serve,
            _dir: dir,
        });
        conns_by_node.push(conns);
    }
    (nodes, conns_by_node)
}

/// Stop accepting and drop every open connection of one node.
fn cut_off(node: &Node, conns: &Conns) {
    node._serve.abort();
    for conn in conns.lock().unwrap().drain(..) {
        conn.abort();
    }
}

#[tokio::test]
async fn partitioned_leader_times_out() {
    let cfg = read_cfg();
    let (nodes, conns) = cuttable_cluster(3, cfg).await;
    let leader = await_leader_with_tick(&nodes, cfg.tick())
        .await
        .expect("a leader is elected");
    nodes[leader].host.propose(vec![1]).await.unwrap();

    // Cut the leader off from both followers.
    for (i, node) in nodes.iter().enumerate() {
        if i != leader {
            cut_off(node, &conns[i]);
        }
    }

    let res = tokio::time::timeout(Duration::from_secs(5), nodes[leader].host.read_index())
        .await
        .expect("read_index returns within 5 seconds");
    match res {
        Err(ReadIndexError::TimedOut) | Err(ReadIndexError::LeadershipLost) => {}
        other => panic!("partitioned leader must not grant an index, got {other:?}"),
    }
}

#[tokio::test]
async fn single_voter_returns_at_once() {
    let dir = TempDir::new().unwrap();
    let sm = TestSm::new();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Os,
    )
    .unwrap();
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], vec![]),
        HashMap::new(),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );

    tokio::time::timeout(Duration::from_secs(10), async {
        while !host.is_leader().await {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the node becomes leader");
    let base = host.status().await.last_index;

    let idx = host.propose(vec![1]).await.unwrap();
    assert_eq!(idx, Index::new(base + 1));

    let read = tokio::time::timeout(Duration::from_secs(2), host.read_index())
        .await
        .expect("single voter answers within 2 seconds");
    assert_eq!(read, Ok(Index::new(base + 1)));
}
