//! Golden bytes for raft-core's serialized and persisted shapes.
//!
//! Every expected value is a literal captured from the pre-P2 code. The
//! newtype-id and private-field changes must leave each one byte-identical.

use std::collections::BTreeMap;
use std::fmt::Debug;

use raft_core::{
    AppendReq, AppendResp, ConfState, EntryKind, InstallSnapshotReq, InstallSnapshotResp,
    Membership, NodeId, PersistedState, PromotionRefused, RaftEntry, TimeoutNowReq,
    TransferRefused, VoteReq, VoteResp,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

fn membership() -> Membership {
    Membership {
        voters: vec![1, 2, 3],
        learners: vec![4],
    }
}

fn joint_conf() -> ConfState {
    ConfState {
        membership: membership(),
        outgoing: Some(vec![1, 2]),
        generation: 7,
    }
}

fn entry(index: u64, kind: EntryKind, command: Vec<u8>) -> RaftEntry {
    RaftEntry {
        term: 2,
        index,
        command,
        kind,
    }
}

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

#[test]
fn membership_and_conf_state_json_are_pinned() {
    pin_json(&membership(), r#"{"voters":[1,2,3],"learners":[4]}"#);
    assert_eq!(
        serde_json::from_str::<Membership>(r#"{"voters":[1,2,3],"learners":[4]}"#).unwrap(),
        membership()
    );
    pin_json(
        &joint_conf(),
        r#"{"membership":{"voters":[1,2,3],"learners":[4]},"outgoing":[1,2],"generation":7}"#,
    );
    assert_eq!(
        serde_json::from_str::<ConfState>(
            r#"{"membership":{"voters":[1,2,3],"learners":[4]},"outgoing":[1,2],"generation":7}"#
        )
        .unwrap(),
        joint_conf()
    );
}

#[test]
fn raft_entry_and_persisted_state_json_are_pinned() {
    let command = entry(5, EntryKind::Command, vec![1, 2, 255]);
    pin_json(
        &command,
        r#"{"term":2,"index":5,"command":[1,2,255],"kind":"Command"}"#,
    );
    let state = PersistedState {
        term: 3,
        voted_for: Some(2),
        log: vec![
            entry(5, EntryKind::Command, vec![9]),
            entry(6, EntryKind::Config, joint_conf().encode()),
        ],
        commit_index: 5,
        snapshot_index: 4,
        snapshot_term: 1,
        snapshot: vec![7, 8],
        conf: Some(joint_conf()),
    };
    let json = concat!(
        r#"{"term":3,"voted_for":2,"log":[{"term":2,"index":5,"command":[9],"kind":"Command"},"#,
        r#"{"term":2,"index":6,"command":[7,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,4,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0],"kind":"Config"}],"#,
        r#""commit_index":5,"snapshot_index":4,"snapshot_term":1,"snapshot":[7,8],"#,
        r#""conf":{"membership":{"voters":[1,2,3],"learners":[4]},"outgoing":[1,2],"generation":7}}"#,
    );
    pin_json(&state, json);
    assert_eq!(serde_json::from_str::<PersistedState>(json).unwrap(), state);
}

#[test]
fn raft_messages_json_are_pinned() {
    pin_json(
        &VoteReq {
            term: 3,
            candidate: 1,
            last_log_index: 9,
            last_log_term: 2,
        },
        r#"{"term":3,"candidate":1,"last_log_index":9,"last_log_term":2}"#,
    );
    pin_json(
        &VoteResp {
            term: 3,
            granted: true,
        },
        r#"{"term":3,"granted":true}"#,
    );
    pin_json(
        &AppendReq {
            term: 3,
            leader: 1,
            prev_log_index: 4,
            prev_log_term: 2,
            entries: vec![entry(5, EntryKind::Command, vec![6])],
            leader_commit: 4,
        },
        r#"{"term":3,"leader":1,"prev_log_index":4,"prev_log_term":2,"entries":[{"term":2,"index":5,"command":[6],"kind":"Command"}],"leader_commit":4}"#,
    );
    pin_json(
        &AppendResp {
            term: 3,
            success: false,
            match_index: 4,
        },
        r#"{"term":3,"success":false,"match_index":4}"#,
    );
    pin_json(
        &InstallSnapshotReq {
            term: 3,
            leader: 1,
            snapshot_index: 8,
            snapshot_term: 2,
            data: vec![1, 2],
        },
        r#"{"term":3,"leader":1,"snapshot_index":8,"snapshot_term":2,"data":[1,2]}"#,
    );
    pin_json(
        &InstallSnapshotResp {
            term: 3,
            accepted: true,
            snapshot_index: 8,
        },
        r#"{"term":3,"accepted":true,"snapshot_index":8}"#,
    );
    pin_json(
        &TimeoutNowReq { term: 3, leader: 1 },
        r#"{"term":3,"leader":1}"#,
    );
}

#[test]
fn refusals_and_node_keyed_maps_json_are_pinned() {
    pin_json(
        &TransferRefused::NotCaughtUp {
            target: 2,
            matched: 4,
            last_index: 9,
        },
        r#"{"NotCaughtUp":{"target":2,"matched":4,"last_index":9}}"#,
    );
    pin_json(
        &PromotionRefused::NotCaughtUp {
            matched: 4,
            target: 9,
        },
        r#"{"NotCaughtUp":{"matched":4,"target":9}}"#,
    );
    let keyed: BTreeMap<NodeId, u64> = [(1, 5), (22, 6)].into_iter().collect();
    assert_eq!(serde_json::to_string(&keyed).unwrap(), r#"{"1":5,"22":6}"#);
    assert_eq!(
        serde_json::from_str::<BTreeMap<NodeId, u64>>(r#"{"1":5,"22":6}"#).unwrap(),
        keyed
    );
}

#[test]
fn conf_state_binary_encoding_is_pinned() {
    let bytes = concat!(
        "0700000000000000",
        "0300000000000000",
        "0100000000000000",
        "0200000000000000",
        "0300000000000000",
        "0100000000000000",
        "0400000000000000",
        "0200000000000000",
        "0100000000000000",
        "0200000000000000",
    );
    assert_eq!(hex(&joint_conf().encode()), bytes);
    assert_eq!(ConfState::decode(&unhex(bytes)), Some(joint_conf()));
}

#[test]
fn debug_output_is_pinned() {
    assert_eq!(
        format!("{:?}", membership()),
        "Membership { voters: [1, 2, 3], learners: [4] }"
    );
    assert_eq!(
        format!(
            "{:?}",
            TransferRefused::NotCaughtUp {
                target: 2,
                matched: 4,
                last_index: 9,
            }
        ),
        "NotCaughtUp { target: 2, matched: 4, last_index: 9 }"
    );
}
