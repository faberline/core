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
use tokio::net::TcpListener;

use crate::support::cluster::{
    await_leader, bind, peers_excluding, wait_for_applied_index, Node, TestSm,
    DEFAULT_APPLIED_INDEX_BUDGET,
};

/// Starts one voter of the three-voter group on `listener`.
///
/// Every accepted peer connection runs in a `JoinSet` owned by the serve task.
/// Aborting the serve task drops the set and aborts every open connection, so
/// no peer keeps talking to the stopped node and its router state is released.
fn start_node(
    id: NodeId,
    listener: TcpListener,
    url: String,
    dir: TempDir,
    all: &[(NodeId, String)],
) -> Node {
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
        let mut conns = tokio::task::JoinSet::new();
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let r = router.clone();
                conns.spawn(async move {
                    let _ = transport_h2c::server::serve_connection(stream, r).await;
                });
            }
        }
    });
    Node {
        host: Arc::new(host),
        sm,
        url,
        _serve: serve,
        _dir: dir,
    }
}

/// Rebinds the listener on the port of `url`, retrying while the old socket
/// is still in use.
async fn rebind(url: &str) -> TcpListener {
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
    let mut last_err = None;
    for _ in 0..50 {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => return l,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    panic!("rebind {url} failed: {last_err:?}");
}

async fn run_stop_and_return(stop_leader: bool) {
    let mut listeners = Vec::new();
    let mut all: Vec<(NodeId, String)> = Vec::new();
    for id in 0..3u64 {
        let (l, url) = bind().await;
        listeners.push(l);
        all.push((NodeId::new(id), url));
    }
    let mut nodes: Vec<Node> = listeners
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let (id, url) = all[i].clone();
            start_node(id, l, url, TempDir::new().unwrap(), &all)
        })
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

    // Stop one node abruptly, with no hand-off: abort its serve task, which
    // closes every open peer connection, then drop the host, which aborts its
    // tick and pump tasks.
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
        _serve: serve,
        _dir: dir,
    } = node;
    serve.abort();
    let _ = serve.await;
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

    let listener = rebind(&url).await;
    let restarted = start_node(NodeId::new(stopped as u64), listener, url, dir, &all);
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
