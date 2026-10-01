//! Semantic code search (Sprint 4 - Track 1)
//!
//! Provides type-aware code search capabilities:
//! - Find similar code patterns
//! - Search by type signature
//! - Find implementations of protocol
//! - Call hierarchy analysis

pub use crate::domain::semantic_search::engine::{
    SemanticSearchEngine, SymbolLocation, TypeLocation,
};
pub use crate::domain::semantic_search::query::{
    CallDirection, SearchKind, SearchQuery, SearchScope, TypeHierarchyDirection,
};
pub use crate::domain::semantic_search::result::{
    MatchContext, MatchKind, SearchMatch, SearchResult, SearchStats,
};
