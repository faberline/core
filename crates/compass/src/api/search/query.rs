//! Query execution for the 6 search modes.
//!
//! Each mode operates on a [`SearchIndex`](super::index::SearchIndex) and returns `SearchResult`.

pub use crate::domain::search::query::call_hierarchy::{search_call_hierarchy, CallGraphIndex};
pub use crate::domain::search::query::documentation::search_documentation;
pub use crate::domain::search::query::implementations::search_implementations;
pub use crate::domain::search::query::similar_code::search_similar_code;
pub use crate::domain::search::query::type_signature::search_by_type_signature;
pub use crate::domain::search::query::usages::search_usages;
