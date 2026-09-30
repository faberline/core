//! Golden JSON for the crate-private peer-wire envelopes.
//!
//! Every expected value is a literal captured from the pre-P2 code. Peers of
//! different versions exchange these bodies, so each must stay byte-identical.
//! These cases pin the peer HTTP handlers' copy; `peer_wire_split` checks that
//! the HTTP peer client's copy encodes the same bytes.

use std::fmt::Debug;

use raft_core::{AppendReq, EntryKind, InstallSnapshotReq, RaftEntry, TimeoutNowReq, VoteReq};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::interfaces::peer_http::wire::{
    AppendEnvelope, CapableSnapEnvelope, CapableSnapshotResp, NotLeader, PublishEnvelope,
    SnapEnvelope, TimeoutNowEnvelope, VoteEnvelope,
};

/// Encode gives `json`, and decoding `json` gives a value whose `Debug`
/// (and re-encoding) matches the original.
fn pin_json<T: Serialize + DeserializeOwned + Debug>(value: &T, json: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), json);
    let decoded: T = serde_json::from_str(json).unwrap();
    assert_eq!(format!("{decoded:?}"), format!("{value:?}"));
    assert_eq!(serde_json::to_string(&decoded).unwrap(), json);
}

fn snapshot_req() -> InstallSnapshotReq {
    InstallSnapshotReq {
        term: 3,
        leader: 1,
        snapshot_index: 8,
        snapshot_term: 2,
        data: vec![1, 2],
    }
}

#[test]
fn vote_append_and_timeout_envelopes_are_pinned() {
    pin_json(
        &VoteEnvelope {
            group_id: "orders".to_owned(),
            from: 1,
            req: VoteReq {
                term: 3,
                candidate: 1,
                last_log_index: 9,
                last_log_term: 2,
            },
        },
        r#"{"group_id":"orders","from":1,"req":{"term":3,"candidate":1,"last_log_index":9,"last_log_term":2}}"#,
    );
    pin_json(
        &AppendEnvelope {
            group_id: "orders".to_owned(),
            from: 1,
            req: AppendReq {
                term: 3,
                leader: 1,
                prev_log_index: 4,
                prev_log_term: 2,
                entries: vec![RaftEntry {
                    term: 3,
                    index: 5,
                    command: vec![6],
                    kind: EntryKind::Command,
                }],
                leader_commit: 4,
            },
        },
        concat!(
            r#"{"group_id":"orders","from":1,"req":{"term":3,"leader":1,"prev_log_index":4,"#,
            r#""prev_log_term":2,"entries":[{"term":3,"index":5,"command":[6],"kind":"Command"}],"#,
            r#""leader_commit":4}}"#,
        ),
    );
    pin_json(
        &TimeoutNowEnvelope {
            group_id: "orders".to_owned(),
            from: 1,
            req: TimeoutNowReq { term: 3, leader: 1 },
        },
        r#"{"group_id":"orders","from":1,"req":{"term":3,"leader":1}}"#,
    );
}

#[test]
fn snapshot_envelopes_and_capable_reply_are_pinned() {
    pin_json(
        &SnapEnvelope {
            group_id: "orders".to_owned(),
            from: 1,
            req: snapshot_req(),
        },
        r#"{"group_id":"orders","from":1,"req":{"term":3,"leader":1,"snapshot_index":8,"snapshot_term":2,"data":[1,2]}}"#,
    );
    pin_json(
        &CapableSnapEnvelope {
            group_id: "orders".to_owned(),
            from: 1,
            req: snapshot_req(),
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
        concat!(
            r#"{"group_id":"orders","from":1,"req":{"term":3,"leader":1,"snapshot_index":8,"#,
            r#""snapshot_term":2,"data":[1,2]},"snapshot_capability":"cap-v1","snapshot_nonce":5}"#,
        ),
    );
    pin_json(
        &CapableSnapshotResp {
            term: 3,
            accepted: true,
            snapshot_index: 8,
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
        r#"{"term":3,"accepted":true,"snapshot_index":8,"snapshot_capability":"cap-v1","snapshot_nonce":5}"#,
    );
}

#[test]
fn publish_envelope_and_not_leader_reply_are_pinned() {
    pin_json(
        &PublishEnvelope {
            group_id: "orders".to_owned(),
            command: vec![1, 2],
        },
        r#"{"group_id":"orders","command":[1,2]}"#,
    );
    let json = r#"{"error":"not-leader","leader":2}"#;
    let reply = NotLeader {
        error: "not-leader",
        leader: Some(2),
    };
    assert_eq!(serde_json::to_string(&reply).unwrap(), json);
    let decoded: NotLeader = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.error, "not-leader");
    assert_eq!(decoded.leader, Some(2));
    let no_leader = NotLeader {
        error: "not-leader",
        leader: None,
    };
    assert_eq!(
        serde_json::to_string(&no_leader).unwrap(),
        r#"{"error":"not-leader","leader":null}"#
    );
}
