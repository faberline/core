//! Own test binary: prints one `CORE_WRITE_PATH_LATENCY` line for
//! `ci/measure-performance.py`. It sets no environment variable, and the
//! threshold lives in the script, not here.

use raft_core::NodeId;
use std::{collections::HashMap, sync::Arc, time::Instant};

use raft_runtime::{FsyncPolicy, HostConfig, Membership, RaftHost, RaftStateMachine, RaftStore};

#[allow(dead_code)]
#[path = "it/support/cluster.rs"]
mod cluster;
use cluster::TestSm;

#[tokio::test]
async fn write_path_latency_is_measured() {
    let data = tempfile::tempdir().unwrap();
    let state_machine = TestSm::new();
    let host = RaftHost::spawn(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0)], Vec::new()),
        HashMap::new(),
        RaftStore::open(
            data.path().to_str().unwrap(),
            NodeId::new(0),
            FsyncPolicy::Always,
        )
        .unwrap(),
        state_machine as Arc<dyn RaftStateMachine>,
        HostConfig::default(),
    );

    for _ in 0..50 {
        host.propose(vec![0u8; 128]).await.unwrap();
    }

    const REQUESTS: usize = 500;
    let mut durations: Vec<f64> = Vec::with_capacity(REQUESTS);
    let total_started = Instant::now();
    for _ in 0..REQUESTS {
        let started = Instant::now();
        host.propose(vec![0u8; 128]).await.unwrap();
        durations.push(started.elapsed().as_secs_f64());
    }
    let total_seconds = total_started.elapsed().as_secs_f64();

    durations.sort_by(f64::total_cmp);
    let p50_ms = durations[REQUESTS / 2] * 1000.0;
    let p99_ms = durations[(REQUESTS * 99) / 100] * 1000.0;
    let rps = REQUESTS as f64 / total_seconds;

    // stderr, like the h2c measurement: stdout carries the harness's
    // `test <name> ... ` prefix, and a line printed there splits the status.
    eprintln!(
        "CORE_WRITE_PATH_LATENCY requests={REQUESTS} rps={rps:.3} p50_ms={p50_ms:.3} p99_ms={p99_ms:.3}"
    );
    assert!(p50_ms > 0.0);
    assert!(rps.is_finite());
}
