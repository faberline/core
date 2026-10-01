use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::entry::{EntryKind, RaftEntry};
use crate::domain::ids::{Index, Term};
use crate::domain::role::Role;

impl RaftNode {
    /// Identity of the next committed entry, without advancing the applied head.
    /// The driver must finish this identity only after its state machine succeeds.
    pub fn peek_next_committed_identity(&self) -> Option<(Index, Term, EntryKind)> {
        let index = self.last_applied.checked_add(1)?;
        if index > self.commit_index {
            return None;
        }
        let offset = index.checked_sub(self.snapshot_index)?.checked_sub(1)?;
        let entry = self.log.get(usize::try_from(offset).ok()?)?;
        (entry.index == index).then_some((index, entry.term, entry.kind))
    }

    /// Finish exactly the pending committed identity. A stale or out-of-order
    /// identity returns false and leaves both application and membership state intact.
    pub fn finish_committed_identity(&mut self, index: Index, term: Term) -> bool {
        let Some((next, next_term, kind)) = self.peek_next_committed_identity() else {
            return false;
        };
        if (next, next_term) != (index, term) {
            return false;
        }
        if kind == EntryKind::Config {
            let offset = (index - self.snapshot_index - 1) as usize;
            if let Some(conf) = ConfState::decode(&self.log[offset].command) {
                self.adopt_conf(conf);
                if self.role == Role::Leader && self.is_joint() {
                    self.check_leave_joint();
                }
            }
        }
        self.last_applied = index;
        true
    }

    /// Newly committed entries (in index order); advances `last_applied`.
    /// Configuration entries are adopted into force and withheld from the
    /// consumer.
    pub fn take_committed(&mut self) -> Vec<RaftEntry> {
        let mut out = Vec::new();
        while self.last_applied < self.commit_index {
            let idx = self.last_applied + 1;
            let pos = (idx - self.snapshot_index - 1) as usize;
            let entry = &self.log[pos];
            if entry.kind == EntryKind::Config {
                if let Some(conf) = ConfState::decode(&entry.command) {
                    self.adopt_conf(conf);
                    if self.role == Role::Leader && self.is_joint() {
                        self.check_leave_joint();
                    }
                }
            } else {
                out.push(entry.clone());
            }
            self.last_applied = idx;
        }
        out
    }
}
