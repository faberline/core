use raft_core::{Index, Membership, NodeId};

/// A small, serializable-in-spirit observation of one node.  It intentionally
/// excludes volatile peer replication details so traces stay stable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeView {
    pub id: NodeId,
    pub role: ConformanceRole,
    pub term: u64,
    pub leader: Option<NodeId>,
    pub commit_index: Index,
    pub last_index: Index,
    pub snapshot_index: Index,
    pub resident_log_entries: usize,
    pub membership: Membership,
    pub joint: bool,
}

/// Deterministic role spelling.  This avoids exposing raft-core's internal
/// role type while still making safety assertions readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConformanceRole {
    Follower,
    Candidate,
    Leader,
}
