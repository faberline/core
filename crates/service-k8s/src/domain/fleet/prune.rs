//! Prune policy for orphaned child objects.

use serde::{Deserialize, Serialize};

/// Policy deciding whether a Fleet prunes owned objects no longer in desired state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PrunePolicy {
    Retain,
    Delete,
}
