use serde::{Deserialize, Serialize};

use super::ids::NodeId;

/// Cluster membership for one Raft group.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    voters: Vec<NodeId>,
    learners: Vec<NodeId>,
}

impl Membership {
    /// Membership with these voters and learners, kept as given: it does not
    /// sort, dedup or validate them. `RaftNode` sorts and dedups the members
    /// it tracks, and the membership-change calls refuse invalid changes.
    pub fn new(voters: Vec<NodeId>, learners: Vec<NodeId>) -> Self {
        Self { voters, learners }
    }

    /// The voting members.
    pub fn voters(&self) -> &[NodeId] {
        &self.voters
    }

    /// The non-voting learners.
    pub fn learners(&self) -> &[NodeId] {
        &self.learners
    }

    /// Move out the voters and the learners, in that order.
    pub fn into_parts(self) -> (Vec<NodeId>, Vec<NodeId>) {
        (self.voters, self.learners)
    }
}

/// Derive membership for node ids `0..n`: voters are the largest **odd** prefix
/// (`n` if odd else `n-1`), the trailing even node becomes a non-voting learner.
/// So the voter count is always odd (1,1,3,3,5,5,…) → clean majorities, and
/// every extra even node is a read-only learner. `n == 0` is treated as 1.
pub fn auto_membership(n: u64) -> Membership {
    let n = n.max(1);
    let voters = if n % 2 == 1 { n } else { n - 1 };
    Membership::new(
        (0..voters).map(NodeId::new).collect(),
        (voters..n).map(NodeId::new).collect(),
    )
}
