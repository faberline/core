//! Semantic search engine with persistent index.
//!
//! Provides 6 search modes over a project-wide inverted index:
//! 1. **ByTypeSignature** - find functions matching a type pattern
//! 2. **CallHierarchy** - BFS callers / callees
//! 3. **Implementations** - find implementors of a trait / protocol
//! 4. **Usages** - find all references to a symbol
//! 5. **SimilarCode** - find structurally similar functions
//! 6. **DocumentationSearch** - keyword search in docstrings

pub mod index;
pub mod query;

pub use crate::application::search::engine::SearchEngine;
