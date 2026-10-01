use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::entry::{EntryKind, RaftEntry};
use crate::domain::ids::{Index, NodeId};
use crate::domain::role::Role;

impl RaftNode {
    /// Append a command on the leader and replicate it. Returns its index, or
    /// `None` if this node is not the leader.
    pub fn propose(&mut self, command: Vec<u8>) -> Option<Index> {
        if self.role != Role::Leader || self.transfer_in_flight.is_some() {
            return None;
        }
        let index = self.last_index() + 1;
        self.resident_log_bytes = self.resident_log_bytes.saturating_add(command.len());
        self.log.push(RaftEntry {
            term: self.current_term,
            index,
            command,
            kind: EntryKind::Command,
        });
        self.broadcast_append();
        self.maybe_commit(); // sole voter commits immediately
        Some(index)
    }

    /// Append a configuration entry on the leader and replicate it. Returns its
    /// index, or `None` if this node is not the leader.
    pub fn propose_config(&mut self, conf: ConfState) -> Option<Index> {
        if self.role != Role::Leader || self.transfer_in_flight.is_some() {
            return None;
        }
        let index = self.last_index() + 1;
        let command = conf.encode();
        self.resident_log_bytes = self.resident_log_bytes.saturating_add(command.len());
        self.log.push(RaftEntry {
            term: self.current_term,
            index,
            command,
            kind: EntryKind::Config,
        });
        self.broadcast_append();
        self.maybe_commit();
        Some(index)
    }

    /// Append a configuration entry adding a learner on the leader and replicate
    /// it. Returns its index, or `None` if this node is not the leader.
    pub fn add_learner(&mut self, peer: NodeId) -> Option<Index> {
        if self.role != Role::Leader || self.transfer_in_flight.is_some() {
            return None;
        }
        let mut conf = self.conf_state.clone();
        conf.generation += 1;
        if !conf.membership.learners.contains(&peer) {
            conf.membership.learners.push(peer);
            conf.membership.learners.sort_unstable();
        }
        self.propose_config(conf)
    }
}
