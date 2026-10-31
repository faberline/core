//! Adoption policy for unowned child objects.

use serde::{Deserialize, Serialize};

/// Policy deciding whether a Fleet adopts unowned observed objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AdoptionPolicy {
    Adopt,
    Refuse,
}
