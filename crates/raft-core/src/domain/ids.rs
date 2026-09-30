use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable node identity (in k8s, the StatefulSet ordinal).
///
/// Build one with [`NodeId::new`] and read the number back with
/// [`NodeId::get`]. It serializes as the bare number (also as a map key),
/// and `{:?}` / `{}` print the bare number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(u64);

impl NodeId {
    /// The node with this number.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The bare number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

pub type Term = u64;
/// 1-based Raft log index; 0 means "before the first entry".
pub type Index = u64;
