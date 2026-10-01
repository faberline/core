//! The peer HTTP handlers (`interfaces::peer_http::wire`) and the HTTP peer
//! client (`infrastructure::peer_wire`) keep separate copies of the peer RPC
//! bodies. Each case builds one body with both copies and checks that they
//! encode the same bytes and decode each other's bytes.

use raft_core::{
    AppendReq, EntryKind, Index, InstallSnapshotReq, NodeId, RaftEntry, Term, TimeoutNowReq,
    VoteReq,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::infrastructure::peer_wire as client;
use crate::interfaces::peer_http::wire as server;

/// `a` and `b` encode the same bytes, and each decodes the other's bytes back
/// to a value that re-encodes to them.
fn same_bytes<A, B>(a: &A, b: &B)
where
    A: Serialize + DeserializeOwned,
    B: Serialize + DeserializeOwned,
{
    let a_json = serde_json::to_string(a).unwrap();
    let b_json = serde_json::to_string(b).unwrap();
    assert_eq!(a_json, b_json);
    let a_from_b: A = serde_json::from_str(&b_json).unwrap();
    let b_from_a: B = serde_json::from_str(&a_json).unwrap();
    assert_eq!(serde_json::to_string(&a_from_b).unwrap(), a_json);
    assert_eq!(serde_json::to_string(&b_from_a).unwrap(), b_json);
}

fn vote_req() -> VoteReq {
    VoteReq {
        term: Term::new(3),
        candidate: NodeId::new(1),
        last_log_index: Index::new(9),
        last_log_term: Term::new(2),
    }
}

fn append_req() -> AppendReq {
    AppendReq {
        term: Term::new(3),
        leader: NodeId::new(1),
        prev_log_index: Index::new(4),
        prev_log_term: Term::new(2),
        entries: vec![RaftEntry {
            term: Term::new(3),
            index: Index::new(5),
            command: vec![6],
            kind: EntryKind::Command,
        }],
        leader_commit: Index::new(4),
    }
}

fn snapshot_req() -> InstallSnapshotReq {
    InstallSnapshotReq {
        term: Term::new(3),
        leader: NodeId::new(1),
        snapshot_index: Index::new(8),
        snapshot_term: Term::new(2),
        data: vec![1, 2],
    }
}

#[test]
fn vote_append_and_timeout_envelopes_match() {
    same_bytes(
        &client::VoteEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: vote_req(),
        },
        &server::VoteEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: vote_req(),
        },
    );
    same_bytes(
        &client::AppendEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: append_req(),
        },
        &server::AppendEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: append_req(),
        },
    );
    same_bytes(
        &client::TimeoutNowEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: TimeoutNowReq {
                term: Term::new(3),
                leader: NodeId::new(1),
            },
        },
        &server::TimeoutNowEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: TimeoutNowReq {
                term: Term::new(3),
                leader: NodeId::new(1),
            },
        },
    );
}

#[test]
fn snapshot_envelopes_and_capable_reply_match() {
    same_bytes(
        &client::SnapEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: snapshot_req(),
        },
        &server::SnapEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: snapshot_req(),
        },
    );
    same_bytes(
        &client::CapableSnapEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: snapshot_req(),
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
        &server::CapableSnapEnvelope {
            group_id: "orders".to_owned(),
            from: NodeId::new(1),
            req: snapshot_req(),
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
    );
    same_bytes(
        &client::CapableSnapshotResp {
            term: 3,
            accepted: true,
            snapshot_index: 8,
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
        &server::CapableSnapshotResp {
            term: 3,
            accepted: true,
            snapshot_index: 8,
            snapshot_capability: "cap-v1".to_owned(),
            snapshot_nonce: 5,
        },
    );
}

#[test]
fn capable_reply_without_accepted_decodes_as_refused_in_both_copies() {
    let json = r#"{"term":3,"snapshot_index":8,"snapshot_capability":"cap-v1","snapshot_nonce":5}"#;
    let from_client: client::CapableSnapshotResp = serde_json::from_str(json).unwrap();
    let from_server: server::CapableSnapshotResp = serde_json::from_str(json).unwrap();
    assert!(!from_client.accepted);
    assert!(!from_server.accepted);
    same_bytes(&from_client, &from_server);
}

#[test]
fn publish_envelope_matches() {
    same_bytes(
        &client::PublishEnvelope {
            group_id: "orders".to_owned(),
            command: vec![1, 2],
        },
        &server::PublishEnvelope {
            group_id: "orders".to_owned(),
            command: vec![1, 2],
        },
    );
}
