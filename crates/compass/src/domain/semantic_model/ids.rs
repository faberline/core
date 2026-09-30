use serde::{Deserialize, Serialize};

/// Unique identifier for a symbol within the semantic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(u64);

impl SymbolId {
    /// Create a new symbol ID
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw id value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Unique identifier for a scope within the semantic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(u64);

impl ScopeId {
    /// Create a new scope ID
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw id value.
    pub const fn get(self) -> u64 {
        self.0
    }
}
