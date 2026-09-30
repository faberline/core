//! The peer RPC bodies this node's HTTP handlers decode and answer with.
//!
//! The HTTP peer client keeps its own copy of the bodies it sends and decodes
//! (`infrastructure/peer_wire.rs`); the `NotLeader` reply lives only here.
//! Peers of different versions exchange these bodies, so both copies must
//! encode the same bytes: `src/tests/peer_wire_split.rs` checks that, and
//! `src/tests/peer_wire_golden.rs` pins this copy.

use raft_core::{AppendReq, InstallSnapshotReq, NodeId, TimeoutNowReq, VoteReq};
use serde::{Deserialize, Serialize};

// --- peer RPC envelopes (the `from` id rides alongside the raft_core message) ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct VoteEnvelope {
    pub(crate) group_id: String,
    pub(crate) from: NodeId,
    pub(crate) req: VoteReq,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct AppendEnvelope {
    pub(crate) group_id: String,
    pub(crate) from: NodeId,
    pub(crate) req: AppendReq,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SnapEnvelope {
    pub(crate) group_id: String,
    pub(crate) from: NodeId,
    pub(crate) req: InstallSnapshotReq,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CapableSnapEnvelope {
    pub(crate) group_id: String,
    pub(crate) from: NodeId,
    pub(crate) req: InstallSnapshotReq,
    pub(crate) snapshot_capability: String,
    pub(crate) snapshot_nonce: u64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CapableSnapshotResp {
    pub(crate) term: u64,
    #[serde(default)]
    pub(crate) accepted: bool,
    pub(crate) snapshot_index: u64,
    pub(crate) snapshot_capability: String,
    pub(crate) snapshot_nonce: u64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TimeoutNowEnvelope {
    pub(crate) group_id: String,
    pub(crate) from: NodeId,
    pub(crate) req: TimeoutNowReq,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PublishEnvelope {
    pub(crate) group_id: String,
    pub(crate) command: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct NotLeader {
    pub(crate) error: &'static str,
    pub(crate) leader: Option<NodeId>,
}
