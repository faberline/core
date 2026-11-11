//! Three real hosts: one voter stops without a hand-off, the live pair keeps
//! committing, and the stopped node restarts from its durable store and
//! catches up on every entry, including those committed while it was down.

use std::sync::Arc;
use std::time::{Duration, Instant};

use raft_core::NodeId;
use raft_runtime::{
    FsyncPolicy, HostConfig, Index, Membership, RaftHost, RaftStateMachine, RaftStore,
};
use tempfile::TempDir;

use crate::support::cluster::{
    await_leader, cluster, peers_excluding, wait_for_applied_index, Node, TestSm,
    DEFAULT_APPLIED_INDEX_BUDGET,
};

/// Rebinds the listener on the port of `url`, reopens the durable store in
/// `dir` and spawns the host again with the default config.
async fn restart_node(id: NodeId, url: &str, dir: TempDir, all: &[(NodeId, String)]) -> Node {
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
    let mut last_err = None;
    let mut listener = None;
    for _ in 0..50 {
        match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    let listener = listener.unwrap_or_else(|| panic!("rebind {url} failed: {last_err:?}"));

    let sm = TestSm::new();
    let store = RaftStore::open(dir.path().to_str().unwrap(), id, FsyncPolicy::Os).unwrap();
    let voters: Vec<NodeId> = (0..3).map(NodeId::new).collect();
    let host = RaftHost::spawn(
        id,
        Membership::new(voters, vec![]),
        peers_excluding(id, all),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );
    let router = host.router();
    let serve = tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let r = router.clone();
                tokio::spawn(async move {
                    let _ = transport_h2c::server::serve_connection(stream, r).await;
                });
            }
        }
    });
    Node {
        host: Arc::new(host),
        sm,
        url: url.to_string(),
        _serve: serve,
        _dir: dir,
    }
}

async fn run_stop_and_return(stop_leader: bool) {
    let mut nodes = cluster(3).await;
    let all: Vec<(NodeId, String)> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (NodeId::new(i as u64), n.url.clone()))
        .collect();

    let leader = await_leader(&nodes)
        .await
        .expect("a three-voter cluster elects a leader");
    let mut before = None;
    for v in 1u8..=3 {
        before = Some(
            nodes[leader]
                .host
                .propose(vec![v])
                .await
                .expect("the leader accepts a proposal"),
        );
    }
    let before = before.unwrap();
    wait_for_applied_index(&nodes, before, DEFAULT_APPLIED_INDEX_BUDGET).await;

    // Stop one node abruptly: abort its serve task and drop the host, which
    // aborts its tick and pump tasks. No hand-off.
    let stopped = if stop_leader {
        leader
    } else {
        (leader + 1) % 3
    };
    let node = nodes.remove(stopped);
    let Node {
        host,
        sm: _sm,
        url,
        _serve,
        _dir,
    } = node;
    _serve.abort();
    drop(_serve);
    drop(host);

    let live_leader = await_leader(&nodes)
        .await
        .expect("the two live nodes keep or elect a leader");
    let mut after = None;
    for v in 4u8..=6 {
        after = Some(
            nodes[live_leader]
                .host
                .propose(vec![v])
                .await
                .expect("the live leader accepts a proposal"),
        );
    }
    let after = after.unwrap();
    wait_for_applied_index(&nodes, after, DEFAULT_APPLIED_INDEX_BUDGET).await;
    assert!(after > before, "the live pair must commit new entries");

    let restarted = restart_node(NodeId::new(stopped as u64), &url, _dir, &all).await;
    nodes.insert(stopped, restarted);
    wait_for_applied_index(&nodes, after, DEFAULT_APPLIED_INDEX_BUDGET).await;

    // The three applied indexes must become equal. Poll with a bounded budget.
    let deadline = Instant::now() + Duration::from_secs(30);
    let values: Vec<Index> = loop {
        let values: Vec<Index> = nodes.iter().map(|n| n.sm.applied_index()).collect();
        if values.iter().all(|v| *v == values[0]) || Instant::now() >= deadline {
            break values;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(values[0], values[1], "applied indexes differ: {values:?}");
    assert_eq!(values[1], values[2], "applied indexes differ: {values:?}");
    assert!(
        values[0] >= after,
        "applied index {:?} is below the last entry committed while the node was down ({after:?})",
        values[0]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_voter_group_survives_a_follower_stop_and_return() {
    run_stop_and_return(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_voter_group_survives_a_leader_stop_and_return() {
    run_stop_and_return(true).await;
}
