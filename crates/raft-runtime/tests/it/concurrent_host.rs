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
    let expected: Vec<Index> = (1..=num_proposals).map(Index::new).collect();
    assert_eq!(
        sorted, expected,
        "indices must be contiguous 1..={num_proposals}"
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    for n in &nodes {
        while n.sm.applied_index() < Index::new(num_proposals) {
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let mut sm_vals = Vec::new();
    let mut watch_vals = Vec::new();
    for n in &nodes {
        sm_vals.push(n.sm.applied_index());
        watch_vals.push(*n.host.applied_watch().borrow());
    }
    println!("sm={sm_vals:?} fresh_watch={watch_vals:?}");

    for sm_val in &sm_vals {
        assert_eq!(*sm_val, Index::new(num_proposals));
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
    let expected: Vec<Index> = (1..=num_proposals).map(Index::new).collect();
    assert_eq!(
        sorted, expected,
        "indices must be contiguous 1..={num_proposals}"
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    for n in &nodes {
        while n.sm.applied_index() < Index::new(num_proposals) {
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let mut sm_vals = Vec::new();
    let mut watch_vals = Vec::new();
    for n in &nodes {
        sm_vals.push(n.sm.applied_index());
        watch_vals.push(*n.host.applied_watch().borrow());
    }
    println!("sm={sm_vals:?} fresh_watch={watch_vals:?}");

    for sm_val in &sm_vals {
        assert_eq!(*sm_val, Index::new(num_proposals));
    }
}
