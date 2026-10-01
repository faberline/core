use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::ids::{Index, NodeId, Term};
use crate::domain::role::Role;

impl RaftNode {
    pub fn conf_state(&self) -> &ConfState {
        &self.conf_state
    }

    /// Whether this node has a joint configuration in force.
    pub fn is_joint(&self) -> bool {
        self.conf_state.outgoing.is_some()
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn role(&self) -> Role {
        self.role
    }
    pub fn is_leader(&self) -> bool {
        self.role == Role::Leader
    }
    pub fn is_voter(&self) -> bool {
        self.is_voter
    }
    pub fn current_term(&self) -> Term {
        self.current_term
    }
    pub fn commit_index(&self) -> Index {
        self.commit_index
    }
    /// Highest log index (covers compacted prefix): `snapshot_index + log.len()`.
    pub fn last_index(&self) -> Index {
        self.snapshot_index + self.log.len() as Index
    }
    /// Last index folded into a snapshot (0 = none).
    pub fn snapshot_index(&self) -> Index {
        self.snapshot_index
    }

    /// Command payload bytes retained in the resident Raft log.
    pub fn resident_log_bytes(&self) -> usize {
        self.resident_log_bytes
    }

    /// Return the term stored at `index` when that prefix is still known.
    ///
    /// Snapshot coordinators use this before they send a prospective snapshot
    /// to peers. `None` means the requested index is outside this node's known
    /// prefix.
    pub fn term_at_index(&self, index: Index) -> Option<Term> {
        if index == 0 || index > self.last_index() {
            return None;
        }
        Some(self.term_at(index))
    }
    /// Number of resident log entries (post-compaction).
    pub fn log_len(&self) -> usize {
        self.log.len()
    }
    /// Last known leader for the current term (for producer redirect).
    pub fn leader(&self) -> Option<NodeId> {
        self.leader_id
    }

    pub(super) fn last_term(&self) -> Term {
        self.log
            .last()
            .map(|e| e.term)
            .unwrap_or(self.snapshot_term)
    }

    /// Term of the entry at `index` (snapshot point or a resident entry).
    pub(super) fn term_at(&self, index: Index) -> Term {
        if index == 0 {
            0
        } else if index <= self.snapshot_index {
            self.snapshot_term
        } else {
            let pos = (index - self.snapshot_index - 1) as usize;
            self.log.get(pos).map(|e| e.term).unwrap_or(0)
        }
    }
}
