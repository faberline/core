//! A full restart of a three-voter group: stop all voters, start them again from
//! their durable stores, and check that the first entry of the new term commits
//! and applies on every voter (#214).

use std::sync::Arc;
use std::time::{Duration, Instant};

use raft_core::NodeId;
use raft_runtime::{
    FsyncPolicy, HostConfig, Index, Membership, RaftHost, RaftStateMachine, RaftStore,
};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::support::cluster::{bind, peers_excluding, TestSm};

const VOTERS: [u64; 3] = [0, 1, 2];

async fn start_voter(
    id: NodeId,
    dir: &std::path::Path,
    listener: TcpListener,
    all: &[(NodeId, String)],
) -> (Arc<RaftHost>, Arc<TestSm>, JoinHandle<()>) {
    let store = RaftStore::open(dir.to_str().unwrap(), id, FsyncPolicy::Os).unwrap();
    let sm = TestSm::new();
    let voters: Vec<NodeId> = VOTERS.iter().map(|v| NodeId::new(*v)).collect();
    let host = Arc::new(RaftHost::spawn(
        id,
        Membership::new(voters, vec![]),
        peers_excluding(id, all),
        store,
        sm.clone() as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    ));
    let router = host.router();
    let serve = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let r = router.clone();
            tokio::spawn(async move {
                let _ = transport_h2c::server::serve_connection(stream, r).await;
            });
        }
    });
    (host, sm, serve)
}

async fn rebind(url: &str) -> TcpListener {
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
    let mut last_err = None;
    for _ in 0..50 {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => return listener,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    panic!("could not rebind {url}: {last_err:?}");
}

async fn await_leader_among(hosts: &[Arc<RaftHost>]) -> usize {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        for (i, host) in hosts.iter().enumerate() {
            if host.is_leader().await {
                return i;
            }
        }
        if Instant::now() >= deadline {
            panic!("no leader after full group restart");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_applied_at_least(sms: &[Arc<TestSm>], at_least: Index, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if sms.iter().all(|sm| sm.applied_index() >= at_least) {
            return;
        }
        if Instant::now() >= deadline {
            let applied: Vec<Index> = sms.iter().map(|sm| sm.applied_index()).collect();
            panic!("{what}: wanted applied >= {at_least:?}, applied indexes {applied:?}");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_applied_equal(sms: &[Arc<TestSm>]) -> Vec<Index> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let applied: Vec<Index> = sms.iter().map(|sm| sm.applied_index()).collect();
        if applied.iter().all(|i| *i == applied[0]) {
            return applied;
        }
        if Instant::now() >= deadline {
            panic!("applied indexes differ after the new term's commit: {applied:?}");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_voter_full_group_restart_keeps_history_and_commits_in_the_new_term() {
    let mut listeners = Vec::new();
    let mut all: Vec<(NodeId, String)> = Vec::new();
    for id in VOTERS {
        let (listener, url) = bind().await;
        all.push((NodeId::new(id), url));
        listeners.push(listener);
    }
    let dirs: Vec<TempDir> = VOTERS.iter().map(|_| TempDir::new().unwrap()).collect();

    // First life: commit three entries on every voter.
    let mut first = Vec::new();
    for (i, listener) in listeners.into_iter().enumerate() {
        let id = NodeId::new(VOTERS[i]);
        first.push(start_voter(id, dirs[i].path(), listener, &all).await);
    }
    let hosts: Vec<Arc<RaftHost>> = first.iter().map(|(h, _, _)| h.clone()).collect();
    let sms: Vec<Arc<TestSm>> = first.iter().map(|(_, sm, _)| sm.clone()).collect();
    let leader = await_leader_among(&hosts).await;
    let mut last = Index::new(0);
    for v in 1..=3u8 {
        last = hosts[leader]
            .propose(vec![v])
            .await
            .expect("the leader accepts a proposal");
    }
    wait_applied_at_least(&sms, last, "first life").await;

    // Stop all three voters and release their listeners and stores.
    drop(hosts);
    drop(sms);
    for (host, sm, serve) in first {
        host.shutdown().await.expect("clean shutdown");
        serve.abort();
        drop(host);
        drop(sm);
    }

    // Second life: same node ids, ports and durable stores, fresh state machines.
    let mut second = Vec::new();
    for (i, (_, url)) in all.iter().enumerate() {
        let listener = rebind(url).await;
        let id = NodeId::new(VOTERS[i]);
        second.push(start_voter(id, dirs[i].path(), listener, &all).await);
    }
    let new_hosts: Vec<Arc<RaftHost>> = second.iter().map(|(h, _, _)| h.clone()).collect();
    let new_sms: Vec<Arc<TestSm>> = second.iter().map(|(_, sm, _)| sm.clone()).collect();
    let new_leader = await_leader_among(&new_hosts).await;

    // The first entry of the new term must commit and apply on all voters.
    let new_last = match tokio::time::timeout(
        Duration::from_secs(8),
        new_hosts[new_leader].propose(vec![4]),
    )
    .await
    {
        Err(_) => panic!("first entry of the new term did not commit: timed out after 8s"),
        Ok(Err(e)) => panic!("first entry of the new term did not commit: {e}"),
        Ok(Ok(idx)) => idx,
    };
    wait_applied_at_least(&new_sms, new_last, "new term").await;
    assert!(
        new_last > last,
        "new term index {new_last:?} must follow {last:?}"
    );
    let applied = wait_applied_equal(&new_sms).await;
    assert_eq!(
        applied.len(),
        3,
        "three voters must report an applied index"
    );

    for (host, _sm, serve) in second {
        host.shutdown().await.expect("clean shutdown");
        serve.abort();
    }
}
