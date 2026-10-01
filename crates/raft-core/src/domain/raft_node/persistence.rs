use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::ids::{Index, NodeId};
use crate::domain::membership::Membership;
use crate::domain::persisted_state::{PersistedState, PersistedStateRef};

impl RaftNode {
    /// Restore a node from durable [`PersistedState`]: term, votedFor, log and
    /// the compaction point are recovered; volatile state (role, commit/apply)
    /// restarts as a Follower at the snapshot point and is re-derived via
    /// replication. Committed entries re-apply idempotently downstream.
    pub fn from_persisted(id: NodeId, membership: &Membership, state: PersistedState) -> RaftNode {
        let conf = state.conf.unwrap_or_else(|| ConfState {
            membership: membership.clone(),
            outgoing: None,
            generation: 0,
        });
        let mut node = RaftNode::new(id, &conf.membership);
        node.conf_state = conf;
        node.current_term = state.term;
        node.voted_for = state.voted_for;
        node.log = state.log;
        node.resident_log_bytes = node.log.iter().map(|entry| entry.command.len()).sum();
        node.snapshot_index = state.snapshot_index;
        node.snapshot_term = state.snapshot_term;
        node.snapshot = state.snapshot;
        node.commit_index = state
            .commit_index
            .max(node.snapshot_index)
            .min(node.last_index());
        node.last_applied = node.snapshot_index;
        if node.snapshot_index > Index::new(0) && !node.snapshot.is_empty() {
            node.installed_snapshot = Some(node.snapshot.clone());
        }
        node
    }

    /// Snapshot the durable hard state for the consumer's store.
    pub fn persisted(&self) -> PersistedState {
        PersistedState {
            term: self.current_term,
            voted_for: self.voted_for,
            log: self.log.clone(),
            commit_index: self.commit_index,
            snapshot_index: self.snapshot_index,
            snapshot_term: self.snapshot_term,
            snapshot: self.snapshot.clone(),
            conf: Some(self.conf_state.clone()),
        }
    }

    /// Borrow the durable hard state without copying the resident log or
    /// snapshot bytes.
    pub fn persisted_ref(&self) -> PersistedStateRef<'_> {
        PersistedStateRef {
            term: self.current_term,
            voted_for: self.voted_for,
            log: &self.log,
            commit_index: self.commit_index,
            snapshot_index: self.snapshot_index,
            snapshot_term: self.snapshot_term,
            snapshot: &self.snapshot,
            conf: Some(&self.conf_state),
        }
    }
}
