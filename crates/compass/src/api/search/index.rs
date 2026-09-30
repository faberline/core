//! Inverted search index for symbol-level code search.
//!
//! Maps symbol names, types, and documentation to file locations,
//! enabling fast lookup across the entire project. Serializable
//! with serde for disk persistence.

pub use crate::domain::search::index::{IndexPosition, IndexSymbolKind, SearchIndex, SymbolEntry};
