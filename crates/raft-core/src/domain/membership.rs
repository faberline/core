use serde::{Deserialize, Serialize};

use super::ids::NodeId;

/// Cluster membership for one Raft group.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub voters: Vec<NodeId>,
    pub learners: Vec<NodeId>,
}

/// Derive membership for node ids `0..n`: voters are the largest **odd** prefix
/// (`n` if odd else `n-1`), the trailing even node becomes a non-voting learner.
/// So the voter count is always odd (1,1,3,3,5,5,…) → clean majorities, and
/// every extra even node is a read-only learner. `n == 0` is treated as 1.
pub fn auto_membership(n: u64) -> Membership {
    let n = n.max(1);
    let voters = if n % 2 == 1 { n } else { n - 1 };
    Membership {
        voters: (0..voters).collect(),
        learners: (voters..n).collect(),
    }
}
