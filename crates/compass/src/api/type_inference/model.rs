//! Semantic Model - Owned, serializable type information independent of AST
//!
//! The SemanticModel provides a persistent representation of resolved types,
//! symbols, and references that can be cached and queried without access to
//! the original source code or AST.

pub use crate::domain::semantic_model::ids::{ScopeId, SymbolId};
pub use crate::domain::semantic_model::model::{ScopeInfo, SemanticModel, TypedRange};
pub use crate::domain::semantic_model::symbol::{SemanticSymbolKind, SymbolData, SymbolReference};
pub use crate::domain::semantic_model::type_info::{LiteralInfo, ParamInfo, TypeInfo};
