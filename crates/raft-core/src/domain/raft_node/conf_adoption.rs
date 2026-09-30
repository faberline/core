use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::entry::EntryKind;
use crate::domain::ids::NodeId;
use crate::domain::role::Role;

impl RaftNode {
    /// Adopt a superseding configuration state. Refuses configurations whose
    /// generation does not strictly exceed the one in force.
    pub fn adopt_conf(&mut self, conf: ConfState) -> bool {
        if conf.generation <= self.conf_state.generation {
            return false;
        }
        let mut members: Vec<NodeId> = conf
            .membership
            .voters()
            .iter()
            .chain(conf.membership.learners().iter())
            .chain(conf.outgoing.iter().flatten())
            .copied()
            .collect();
        members.sort_unstable();
        members.dedup();
        self.peers = members.into_iter().filter(|m| *m != self.id).collect();
        self.is_voter = conf.membership.voters().contains(&self.id)
            || conf
                .outgoing
                .as_ref()
                .map_or(false, |o| o.contains(&self.id));
        self.conf_state = conf;
        for l in self.conf_state.membership.learners() {
            self.learner_read_targets
                .entry(*l)
                .or_insert(self.commit_index);
        }
        if self.role == Role::Leader {
            let next = self.last_index() + 1;
            for p in &self.peers {
                self.next_index.entry(*p).or_insert(next);
                self.match_index.entry(*p).or_insert(0);
            }
        }
        true
    }

    pub(super) fn check_leave_joint(&mut self) {
        if self.role != Role::Leader || !self.is_joint() {
            return;
        }
        let has_pending = self.log.iter().any(|e| {
            e.index > self.commit_index
                && e.kind == EntryKind::Config
                && e.term == self.current_term
        });
        if !has_pending {
            let conf = ConfState {
                membership: self.conf_state.membership.clone(),
                outgoing: None,
                generation: self.conf_state.generation + 1,
            };
            self.propose_config(conf);
        }
    }
}
