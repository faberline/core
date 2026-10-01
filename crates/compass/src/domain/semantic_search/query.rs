use std::path::PathBuf;

use crate::type_inference::Type;

// ============================================================================
// Search Query
// ============================================================================

/// A semantic search query.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Type of search
    pub kind: SearchKind,
    /// Scope to search in
    pub scope: SearchScope,
    /// Maximum results
    pub max_results: usize,
}

/// Type of semantic search.
#[derive(Debug, Clone)]
pub enum SearchKind {
    /// Find by type signature
    ByTypeSignature {
        params: Vec<Type>,
        return_type: Option<Type>,
    },
    /// Find implementations of protocol/interface
    Implementations { protocol: String },
    /// Find usages of a symbol
    Usages { symbol: String, file: PathBuf },
    /// Find similar code patterns
    SimilarPatterns { pattern: String },
    /// Find by documentation content
    ByDocumentation { query: String },
    /// Find call hierarchy (callers or callees)
    CallHierarchy {
        symbol: String,
        file: PathBuf,
        direction: CallDirection,
    },
    /// Find type hierarchy (supertypes or subtypes)
    TypeHierarchy {
        type_name: String,
        direction: TypeHierarchyDirection,
    },
}

/// Direction for call hierarchy search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDirection {
    /// Find callers (incoming calls)
    Callers,
    /// Find callees (outgoing calls)
    Callees,
}

/// Direction for type hierarchy search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeHierarchyDirection {
    /// Find supertypes (parents)
    Supertypes,
    /// Find subtypes (children)
    Subtypes,
    /// Find both
    Both,
}

/// Scope for search operations.
#[derive(Debug, Clone)]
pub enum SearchScope {
    /// Current file only
    CurrentFile(PathBuf),
    /// Specific files
    Files(Vec<PathBuf>),
    /// Entire project
    Project,
    /// Project with dependencies
    ProjectWithDeps,
}
