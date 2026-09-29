use serde::{Deserialize, Serialize};

/// Unique identifier for a symbol within the semantic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u64);

impl SymbolId {
    /// Create a new symbol ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique identifier for a scope within the semantic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(pub u64);

impl ScopeId {
    /// Create a new scope ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}
