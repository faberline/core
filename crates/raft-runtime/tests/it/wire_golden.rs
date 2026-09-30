//! Golden bytes for raft-runtime's public serialized and persisted shapes.
//!
//! Every expected value is a literal captured from the pre-P2 code. The
//! newtype-id, private-field and layering changes must leave each one
//! byte-identical. The peer-wire envelopes are crate-private and are pinned by
//! the unit tests in `src/tests/peer_wire_golden.rs`.

use raft_core::NodeId;
use std::fmt::Debug;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use raft_core::{ConfState, EntryKind, Index, Membership, PersistedState, RaftEntry, Term};
use raft_runtime::conformance::DeterministicHost;
use raft_runtime::{
    ActiveAssignment, AdmissionRefused, AssignmentEpoch, ClusterStateView, FenceToken,
    FencedAssignment, FsyncPolicy, GroupId, LeadershipHandoff, MembershipPhase, PeerAddr, RaftRole,
    RaftStateMachine, RaftStatus, RaftStore, StateMachineError,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tempfile::TempDir;

/// Encode gives `json`, and decoding `json` gives a value whose `Debug`
/// (and re-encoding) matches the original.
fn pin_json<T: Serialize + DeserializeOwned + Debug>(value: &T, json: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), json);
    let decoded: T = serde_json::from_str(json).unwrap();
    assert_eq!(format!("{decoded:?}"), format!("{value:?}"));
    assert_eq!(serde_json::to_string(&decoded).unwrap(), json);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn membership() -> Membership {
    Membership::new(
        vec![NodeId::new(1), NodeId::new(2), NodeId::new(3)],
        vec![NodeId::new(4)],
    )
}

fn joint_conf() -> ConfState {
    ConfState {
        membership: membership(),
        outgoing: Some(vec![NodeId::new(1), NodeId::new(2)]),
        generation: 7,
    }
}

fn persisted_state() -> PersistedState {
    PersistedState {
        term: Term::new(3),
        voted_for: Some(NodeId::new(2)),
        log: vec![
            RaftEntry {
                term: Term::new(2),
                index: Index::new(5),
                command: vec![9],
                kind: EntryKind::Command,
            },
            RaftEntry {
                term: Term::new(3),
                index: Index::new(6),
                command: joint_conf().encode(),
                kind: EntryKind::Config,
            },
        ],
        commit_index: Index::new(5),
        snapshot_index: Index::new(4),
        snapshot_term: Term::new(1),
        snapshot: vec![7, 8],
        conf: Some(joint_conf()),
    }
}

const STATE_FILE: &str = "raft-7.state";
const STATE_HEX: &str = concat!(
    "5241465453543034030000000000000001020000000000000005000000000000",
    "00040000000000000001000000000000000200000000000000bd7c250566c6e9",
    "9f47c174b589b7551f8b0e930ed056511d1e8f653bc71d3c4a01070000000000",
    "0000030000000000000001000000000000000200000000000000030000000000",
    "0000010000000000000004000000000000000200000000000000010000000000",
    "00000200000000000000012823625bb4e8bbb03b9d7ba081cbc2d16875c1fb1a",
    "c1c4be7c324fdd1faa1c4ca30000000000000002000000000000000500000000",
    "000000060000000000000003000000000000009303a5ea9aeb00ca7c9f5f4673",
    "f7be2414b9bfa81aefcfc24c82bf69a30ca629",
);
const LOG_FILE: &str =
    "raft-7-log-2823625bb4e8bbb03b9d7ba081cbc2d16875c1fb1ac1c4be7c324fdd1faa1c4c.artifact";
const LOG_HEX: &str = concat!(
    "524146544c4730311a000000000000005d38fb30020000000000000005000000",
    "000000000001000000000000000969000000000000003a14cf70030000000000",
    "0000060000000000000001500000000000000007000000000000000300000000",
    "0000000100000000000000020000000000000003000000000000000100000000",
    "0000000400000000000000020000000000000001000000000000000200000000",
    "000000",
);
const SNAPSHOT_FILE: &str = "raft-7-snap-4-1.artifact";

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn durable_state_log_and_snapshot_bytes_are_pinned() {
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(7),
        FsyncPolicy::Os,
    )
    .unwrap();
    store.save(&persisted_state()).unwrap();
    assert_eq!(
        file_names(dir.path()),
        vec![
            LOG_FILE.to_owned(),
            SNAPSHOT_FILE.to_owned(),
            STATE_FILE.to_owned()
        ]
    );
    assert_eq!(store.path().file_name().unwrap(), STATE_FILE);
    let read = |name: &str| std::fs::read(dir.path().join(name)).unwrap();
    assert_eq!(hex(&read(STATE_FILE)), STATE_HEX);
    assert_eq!(hex(&read(LOG_FILE)), LOG_HEX);
    assert_eq!(read(SNAPSHOT_FILE), vec![7, 8]);

    let restored = TempDir::new().unwrap();
    std::fs::write(restored.path().join(STATE_FILE), unhex(STATE_HEX)).unwrap();
    std::fs::write(restored.path().join(LOG_FILE), unhex(LOG_HEX)).unwrap();
    std::fs::write(restored.path().join(SNAPSHOT_FILE), [7, 8]).unwrap();
    let store = RaftStore::open(
        restored.path().to_str().unwrap(),
        NodeId::new(7),
        FsyncPolicy::Os,
    )
    .unwrap();
    assert_eq!(store.load().unwrap(), Some(persisted_state()));
}

#[test]
fn named_group_state_file_name_is_pinned() {
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open_group(
        dir.path().to_str().unwrap(),
        NodeId::new(7),
        GroupId("orders".to_owned()),
        FsyncPolicy::Os,
    )
    .unwrap();
    assert_eq!(
        store.path().file_name().unwrap(),
        "raft-7-6f7264657273.state"
    );
    pin_json(&GroupId("orders".to_owned()), r#""orders""#);
}

#[test]
fn raft_status_json_is_pinned() {
    let status = RaftStatus {
        group_id: "orders".to_owned(),
        id: NodeId::new(1),
        role: "Leader".to_owned(),
        term: 3,
        commit_index: 9,
        last_index: 10,
        snapshot_index: 4,
        applied_index: 8,
        leader: Some(NodeId::new(1)),
        is_leader: true,
        durability_error: None,
        committed_voters: vec![NodeId::new(1), NodeId::new(2), NodeId::new(3)],
        incoming_voters: Some(vec![NodeId::new(1), NodeId::new(2)]),
        learners: vec![NodeId::new(4)],
        membership_phase: MembershipPhase::Joint,
        undeliverable_never_addressed: 5,
        undeliverable_withdrawn_address: 6,
        proposal_rejected_before_routing: 7,
        proposal_rejected_before_append: 8,
        proposal_admission_closed: false,
        lifecycle_generation: 2,
        snapshot_capability: Some("cap-v1".to_owned()),
        resident_log_bytes: 64,
        max_resident_log_bytes: 1024,
    };
    pin_json(
        &status,
        concat!(
            r#"{"group_id":"orders","id":1,"role":"Leader","term":3,"commit_index":9,"#,
            r#""last_index":10,"snapshot_index":4,"applied_index":8,"leader":1,"is_leader":true,"#,
            r#""durability_error":null,"committed_voters":[1,2,3],"incoming_voters":[1,2],"#,
            r#""learners":[4],"membership_phase":"Joint","undeliverable_never_addressed":5,"#,
            r#""undeliverable_withdrawn_address":6,"proposal_rejected_before_routing":7,"#,
            r#""proposal_rejected_before_append":8,"proposal_admission_closed":false,"#,
            r#""lifecycle_generation":2,"snapshot_capability":"cap-v1","resident_log_bytes":64,"#,
            r#""max_resident_log_bytes":1024}"#,
        ),
    );
}

#[test]
fn assignment_and_refusal_json_are_pinned() {
    let token = FenceToken {
        owner: NodeId::new(2),
        epoch: AssignmentEpoch::new(5),
    };
    pin_json(&token, r#"{"owner":2,"epoch":5}"#);
    let active = ActiveAssignment {
        token,
        expires_at_ms: 1000,
    };
    pin_json(
        &active,
        r#"{"token":{"owner":2,"epoch":5},"expires_at_ms":1000}"#,
    );
    let mut fenced = FencedAssignment::idle();
    fenced.assign(NodeId::new(2), 10, 1000).unwrap();
    pin_json(
        &fenced,
        r#"{"epoch":1,"active":{"token":{"owner":2,"epoch":1},"expires_at_ms":1000}}"#,
    );
    assert_eq!(
        fenced
            .assign(NodeId::new(3), 20, 2000)
            .unwrap_err()
            .to_string(),
        "assignment is owned by node 2 at epoch 1 until 1000"
    );
    pin_json(
        &AdmissionRefused::Unroutable {
            target: NodeId::new(4),
        },
        r#"{"Unroutable":{"target":4}}"#,
    );
    pin_json(
        &LeadershipHandoff::Transferred {
            target: NodeId::new(2),
        },
        r#"{"Transferred":{"target":2}}"#,
    );
}

#[test]
fn cluster_view_json_is_pinned() {
    let peer = PeerAddr {
        pod_name: "orders-1".to_owned(),
        host: "orders-1.orders".to_owned(),
        raft_port: 7001,
        client_port: 8001,
        role: RaftRole::Follower,
    };
    pin_json(
        &peer,
        r#"{"pod_name":"orders-1","host":"orders-1.orders","raft_port":7001,"client_port":8001,"role":"follower"}"#,
    );
    let view = ClusterStateView {
        pod_name: "orders-0".to_owned(),
        shard_index: 0,
        replica_index: 1,
        role: RaftRole::Leader,
        peers: vec![peer],
        applied_index: 9,
        leader_term: 3,
        replication_lag_ms: 12,
    };
    pin_json(
        &view,
        concat!(
            r#"{"pod_name":"orders-0","shard_index":0,"replica_index":1,"role":"leader","#,
            r#""peers":[{"pod_name":"orders-1","host":"orders-1.orders","raft_port":7001,"#,
            r#""client_port":8001,"role":"follower"}],"applied_index":9,"leader_term":3,"#,
            r#""replication_lag_ms":12}"#,
        ),
    );
}

struct CountingSm(AtomicU64);

impl RaftStateMachine for CountingSm {
    fn apply(&self, index: Index, _: &[u8]) -> Result<(), StateMachineError> {
        self.0.store(index.get(), Ordering::Release);
        Ok(())
    }
    fn snapshot(&self, writer: &mut dyn Write) -> Result<(), StateMachineError> {
        writer
            .write_all(&self.0.load(Ordering::Acquire).to_le_bytes())
            .map_err(StateMachineError::other)?;
        Ok(())
    }
    fn restore(&self, reader: &mut dyn Read) -> Result<(), StateMachineError> {
        let mut bytes = [0; 8];
        reader
            .read_exact(&mut bytes)
            .map_err(StateMachineError::other)?;
        self.0.store(u64::from_le_bytes(bytes), Ordering::Release);
        Ok(())
    }
    fn applied_index(&self) -> Index {
        Index::new(self.0.load(Ordering::Acquire))
    }
}

#[test]
fn conformance_envelope_meta_and_node_view_are_pinned() {
    let dir = TempDir::new().unwrap();
    let store = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Os,
    )
    .unwrap();
    let mut host = DeterministicHost::open(
        NodeId::new(0),
        Membership::new(vec![NodeId::new(0), NodeId::new(1), NodeId::new(2)], vec![]),
        store,
        Arc::new(CountingSm(AtomicU64::new(0))),
    )
    .unwrap();
    while host.ready_peers().is_empty() {
        host.tick().unwrap();
    }
    let envelope = host.take_next(NodeId::new(1)).unwrap();
    assert_eq!(
        format!("{:?}", envelope.meta()),
        concat!(
            "EnvelopeMeta { id: 1, from: 0, to: 1, kind: Vote, fingerprint: ",
            "\"a88288886feb6acc319ace669f1bf209654327b25e02eccba565d65f38630e0d\" }",
        )
    );
    assert_eq!(
        format!("{:?}", host.view()),
        concat!(
            "NodeView { id: 0, role: Candidate, term: 1, leader: None, commit_index: 0, ",
            "last_index: 0, snapshot_index: 0, resident_log_entries: 0, ",
            "membership: Membership { voters: [0, 1, 2], learners: [] }, joint: false }",
        )
    );
}
