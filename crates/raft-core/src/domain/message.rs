use serde::{Deserialize, Serialize};

use super::entry::RaftEntry;
use super::ids::{Index, NodeId, Term};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteReq {
    pub term: Term,
    pub candidate: NodeId,
    pub last_log_index: Index,
    pub last_log_term: Term,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteResp {
    pub term: Term,
    pub granted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendReq {
    pub term: Term,
    pub leader: NodeId,
    pub prev_log_index: Index,
    pub prev_log_term: Term,
    pub entries: Vec<RaftEntry>,
    pub leader_commit: Index,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendResp {
    pub term: Term,
    pub success: bool,
    /// Highest log index the follower now matches the leader on.
    pub match_index: Index,
}

/// Ship a state-machine snapshot to a follower whose needed entries have been
/// compacted away. `data` is opaque (the consumer's serialized state machine).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallSnapshotReq {
    pub term: Term,
    pub leader: NodeId,
    pub snapshot_index: Index,
    pub snapshot_term: Term,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallSnapshotResp {
    pub term: Term,
    /// True only when this voter accepted the requested snapshot identity, or
    /// already holds a snapshot that supersedes it. Older peers omit this
    /// field and therefore fail closed.
    #[serde(default)]
    pub accepted: bool,
    /// The snapshot index the follower now holds.
    pub snapshot_index: Index,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutNowReq {
    pub term: Term,
    pub leader: NodeId,
}

#[derive(Clone, Debug)]
pub enum RaftMsg {
    Vote(VoteReq),
    VoteResp(VoteResp),
    Append(AppendReq),
    AppendResp(AppendResp),
    InstallSnapshot(InstallSnapshotReq),
    InstallSnapshotResp(InstallSnapshotResp),
    TimeoutNow(TimeoutNowReq),
}

/// A message the driver must deliver to node `to`.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub to: NodeId,
    pub msg: RaftMsg,
}
