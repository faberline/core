//! Prune policy for orphaned child objects.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Policy deciding whether a Fleet prunes owned objects no longer in desired state.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub enum PrunePolicy {
    /// Keep owned objects that are no longer desired, and report them as orphans.
    #[default]
    Retain,
    Delete,
}
