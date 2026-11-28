//! Own test binary: it prints one `CORE_WRITE_PATH_LATENCY` line that
//! `ci/measure-performance.py` parses. It starts one single-voter host and
//! sets no environment variable, so it does not share its process with tests
//! that change the host configuration.

use raft_core::NodeId;
use std::{collections::HashMap, sync::Arc, time::Instant};

use raft_runtime::{FsyncPolicy, HostConfig, Membership, RaftHost, RaftStateMachine, RaftStore};

#[allow(dead_code)]
#[path = "it/support/cluster.rs"]
mod cluster;
use cluster::TestSm;

const WARM_UP_REQUESTS: usize = 50;
const TIMED_REQUESTS: usize = 5000;
const PAYLOAD_BYTES: usize = 128;

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

    for _ in 0..WARM_UP_REQUESTS {
        host.propose(vec![0u8; PAYLOAD_BYTES]).await.unwrap();
    }

    let mut durations = Vec::with_capacity(TIMED_REQUESTS);
    let started = Instant::now();
    for _ in 0..TIMED_REQUESTS {
        let call = Instant::now();
        host.propose(vec![0u8; PAYLOAD_BYTES]).await.unwrap();
        durations.push(call.elapsed().as_secs_f64());
    }
    let total_seconds = started.elapsed().as_secs_f64();

    durations.sort_by(f64::total_cmp);
    let p50_ms = durations[TIMED_REQUESTS / 2] * 1000.0;
    let p99_ms = durations[(TIMED_REQUESTS * 99) / 100] * 1000.0;
    let rps = TIMED_REQUESTS as f64 / total_seconds;

    // stderr, like CORE_SNAPSHOT_PEAK: on stdout the line would join the
    // `test ... ` status line of the harness, which the script needs intact.
    eprintln!(
        "CORE_WRITE_PATH_LATENCY requests={TIMED_REQUESTS} rps={rps:.3} p50_ms={p50_ms:.3} p99_ms={p99_ms:.3}"
    );
    assert!(p50_ms > 0.0, "p50 must be positive");
    assert!(rps.is_finite(), "throughput must be finite");
}
