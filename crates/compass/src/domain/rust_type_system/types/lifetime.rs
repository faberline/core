// ============================================================================
// Lifetime Types
// ============================================================================

/// Unique identifier for lifetimes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifetimeId(pub usize);

/// Rust lifetime representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Lifetime {
    /// Named lifetime ('a, 'b, etc.)
    Named { id: LifetimeId, name: String },
    /// Static lifetime ('static)
    Static,
    /// Anonymous/elided lifetime ('_)
    Anonymous,
    /// Inferred lifetime (not yet resolved)
    Inferred(LifetimeId),
}

impl Lifetime {
    /// Create a new named lifetime
    pub fn named(id: LifetimeId, name: impl Into<String>) -> Self {
        Lifetime::Named {
            id,
            name: name.into(),
        }
    }

    /// Check if this lifetime outlives another
    pub fn outlives(&self, other: &Lifetime) -> bool {
        match (self, other) {
            (Lifetime::Static, _) => true,
            (_, Lifetime::Static) => false,
            (Lifetime::Named { id: a, .. }, Lifetime::Named { id: b, .. }) => a == b,
            _ => false, // Conservative: unknown relationships
        }
    }
}
