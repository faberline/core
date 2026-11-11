use raft_runtime::{HostConfig, Index, RaftStateMachine};
use std::time::Duration;

use crate::support::cluster;
use cluster::*;

#[tokio::test]
async fn concurrent() {
    // Index contiguity is under test, not latency. The first of 256 queued
    // proposals can outlast the default 10s propose budget when other clusters
    // share the CPU, which fails them as Ambiguous although every entry applies.
    let cfg = HostConfig::default().with_propose_timeout(Duration::from_secs(60));
    let nodes = cluster_with_config(3, cfg).await;
    let leader = await_leader(&nodes).await.expect("a leader is elected");

    let num_proposals = 256u64;
    let mut set = tokio::task::JoinSet::new();
    for v in 1..=num_proposals {
        let host = nodes[leader].host.clone();
        set.spawn(async move { host.propose(v.to_le_bytes().to_vec()).await.unwrap() });
    }

    let mut returned_indices = Vec::new();
    while let Some(res) = set.join_next().await {
        returned_indices.push(res.unwrap());
    }

    let mut distinct = returned_indices.clone();
    distinct.sort_unstable();
    distinct.dedup();
    println!("issued {num_proposals} distinct {}", distinct.len());

    let mut sorted = returned_indices.clone();
    sorted.sort_unstable();
    let first = *sorted.first().unwrap();
    let expected: Vec<Index> = (first.get()..first.get() + num_proposals)
        .map(Index::new)
        .collect();
    assert_eq!(
        sorted, expected,
        "indices must be contiguous for {num_proposals} proposals"
    );
    let last = Index::new(first.get() + num_proposals - 1);

    wait_for_applied_index(&nodes, last, DEFAULT_APPLIED_INDEX_BUDGET).await;

    let mut sm_vals = Vec::new();
    let mut watch_vals = Vec::new();
    for n in &nodes {
        sm_vals.push(n.sm.applied_index());
        watch_vals.push(*n.host.applied_watch().borrow());
    }
    println!("sm={sm_vals:?} fresh_watch={watch_vals:?}");

    for sm_val in &sm_vals {
        assert_eq!(*sm_val, last);
    }
}

#[tokio::test]
async fn sequential() {
    let nodes = cluster(3).await;
    let leader = await_leader(&nodes).await.expect("a leader is elected");

    let num_proposals = 256u64;
    let mut returned_indices = Vec::new();
    for v in 1..=num_proposals {
        let idx = nodes[leader]
            .host
            .propose(v.to_le_bytes().to_vec())
            .await
            .unwrap();
        returned_indices.push(idx);
    }

    let mut distinct = returned_indices.clone();
    distinct.sort_unstable();
    distinct.dedup();
    println!("issued {num_proposals} distinct {}", distinct.len());

    let mut sorted = returned_indices.clone();
    sorted.sort_unstable();
    let first = *sorted.first().unwrap();
    let expected: Vec<Index> = (first.get()..first.get() + num_proposals)
        .map(Index::new)
        .collect();
    assert_eq!(
        sorted, expected,
        "indices must be contiguous for {num_proposals} proposals"
    );
    let last = Index::new(first.get() + num_proposals - 1);

    wait_for_applied_index(&nodes, last, DEFAULT_APPLIED_INDEX_BUDGET).await;

    let mut sm_vals = Vec::new();
    let mut watch_vals = Vec::new();
    for n in &nodes {
        sm_vals.push(n.sm.applied_index());
        watch_vals.push(*n.host.applied_watch().borrow());
    }
    println!("sm={sm_vals:?} fresh_watch={watch_vals:?}");

    for sm_val in &sm_vals {
        assert_eq!(*sm_val, last);
    }
}

#[tokio::test]
async fn wait_for_applied_index_times_out_by_name() {
    // Run the wait in a spawned task so its panic is captured as a JoinError
    // and the payload can be asserted. `#[should_panic]` is not used: CI's
    // contract roster check cannot match the executed name for it.
    let handle = tokio::spawn(async {
        let nodes = cluster(3).await;
        wait_for_applied_index(&nodes, Index::new(1), Duration::from_millis(50)).await;
    });
    let err = handle
        .await
        .expect_err("an idle cluster must not reach applied index 1");
    assert!(err.is_panic(), "the wait must fail by panicking");
    let payload = err.into_panic();
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("the timeout panic carries a string message");
    assert!(
        msg.contains("timed out waiting for node 0"),
        "unexpected panic message: {msg}"
    );
}
