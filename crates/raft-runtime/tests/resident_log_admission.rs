//! Own test binary: this file sets `RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES`,
//! which `RaftHost` reads on every spawn, so a host started by any other test
//! in the same process would inherit the 64-byte resident-log limit.

use raft_core::NodeId;
use std::{collections::HashMap, sync::Arc};

use raft_core::{EntryKind, Index, PersistedState, RaftEntry, Term};
use raft_runtime::{FsyncPolicy, HostConfig, Membership, RaftHost, RaftStateMachine, RaftStore};

#[allow(dead_code)]
#[path = "it/support/cluster.rs"]
mod cluster;
use cluster::TestSm;

#[tokio::test]
async fn resident_log_limit_backpressures_then_reopens_after_compaction() {
    std::env::set_var("RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES", "64");
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

    assert_eq!(host.propose(vec![1; 40]).await.unwrap(), Index::new(1));
    let error = host.propose(vec![2; 40]).await.unwrap_err();
    assert!(error.to_string().contains("resident log memory limit"));
    host.snapshot_and_compact_through(Index::new(1))
        .await
        .unwrap();
    assert_eq!(host.propose(vec![3; 40]).await.unwrap(), Index::new(2));
    std::env::remove_var("RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES");
}

#[tokio::test]
async fn corrupt_referenced_v4_log_refuses_host_startup() {
    let data = tempfile::tempdir().unwrap();
    let store = RaftStore::open(
        data.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Always,
    )
    .unwrap();
    store
        .save(&PersistedState {
            term: Term::new(1),
            voted_for: Some(NodeId::new(0)),
            log: vec![RaftEntry {
                term: Term::new(1),
                index: Index::new(1),
                command: vec![7; 128],
                kind: EntryKind::Command,
            }],
            commit_index: Index::new(1),
            snapshot_index: Index::new(0),
            snapshot_term: Term::new(0),
            snapshot: Vec::new(),
            conf: None,
        })
        .unwrap();
    let artifact = std::fs::read_dir(data.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("-log-") && name.ends_with(".artifact"))
        })
        .expect("V4 log artifact");
    let length = std::fs::metadata(&artifact).unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&artifact)
        .unwrap()
        .set_len(length / 2)
        .unwrap();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        RaftHost::spawn(
            NodeId::new(0),
            Membership::new(vec![NodeId::new(0)], Vec::new()),
            HashMap::new(),
            store,
            TestSm::new() as Arc<dyn RaftStateMachine>,
            HostConfig::default(),
        )
    }));
    assert!(result.is_err(), "corrupt V4 log must refuse host startup");
}
