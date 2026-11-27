//! Adoption policy for unowned child objects.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Policy deciding whether a Fleet adopts unowned observed objects.
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
pub enum AdoptionPolicy {
    Adopt,
    /// Refuse to touch unowned objects. The safe default.
    #[default]
    Refuse,
}
