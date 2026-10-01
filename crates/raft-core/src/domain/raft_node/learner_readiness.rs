use super::RaftNode;
use crate::domain::ids::{Index, NodeId};
use crate::domain::role::Role;

impl RaftNode {
    /// Highest index this leader has recorded as replicated to `peer`, or `None`
    /// if this node is not the leader or `peer` is not an admitted learner.
    pub fn learner_matched(&self, peer: NodeId) -> Option<Index> {
        if self.role != Role::Leader || !self.conf_state.membership.learners().contains(&peer) {
            return None;
        }
        self.match_index.get(&peer).copied().or(Some(Index::new(0)))
    }

    /// The index `peer` must replicate to before it is fit to serve reads, or
    /// `None` if `peer` is not an admitted learner.
    pub fn learner_read_target(&self, peer: NodeId) -> Option<Index> {
        if !self.conf_state.membership.learners().contains(&peer) {
            return None;
        }
        self.learner_read_targets.get(&peer).copied()
    }

    /// Whether an admitted learner has caught up to its recorded read target,
    /// or `None` if this node is not the leader or `peer` is not an admitted learner.
    pub fn learner_read_eligible(&self, peer: NodeId) -> Option<bool> {
        let matched = self.learner_matched(peer)?;
        let target = self.learner_read_target(peer)?;
        Some(matched >= target)
    }
}
