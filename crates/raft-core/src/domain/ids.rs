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

/// A Raft term. `Term::default()` is term 0, before any election.
///
/// Build one with [`Term::new`] and read the number back with [`Term::get`].
/// It serializes as the bare number, and `{:?}` / `{}` print the bare number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Term(u64);

impl Term {
    /// The term with this number.
    pub const fn new(term: u64) -> Self {
        Self(term)
    }

    /// The bare number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The term after this one.
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A 1-based Raft log index; `Index::default()` (0) means "before the first
/// entry".
///
/// Build one with [`Index::new`] and read the number back with
/// [`Index::get`]. It serializes as the bare number, and `{:?}` / `{}` print
/// the bare number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Index(u64);

impl Index {
    /// The index with this number.
    pub const fn new(index: u64) -> Self {
        Self(index)
    }

    /// The bare number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The index after this one.
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The index before this one. Callers make sure it is above 0.
    pub const fn prev(self) -> Self {
        Self(self.0 - 1)
    }

    /// The index `n` entries before this one, stopping at 0.
    pub const fn saturating_sub(self, n: u64) -> Self {
        Self(self.0.saturating_sub(n))
    }

    /// The index `n` entries after this one, stopping at the largest index.
    pub const fn saturating_add(self, n: u64) -> Self {
        Self(self.0.saturating_add(n))
    }

    /// The index `n` entries after this one, or `None` past the largest
    /// index.
    pub const fn checked_add(self, n: u64) -> Option<Self> {
        match self.0.checked_add(n) {
            Some(index) => Some(Self(index)),
            None => None,
        }
    }
}

macro_rules! bare_number_fmt {
    ($($ty:ty),*) => {$(
        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    )*};
}

bare_number_fmt!(NodeId, Term, Index);
