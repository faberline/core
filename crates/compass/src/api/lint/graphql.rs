//! GraphQL lint checker — AST-based via tree-sitter-graphql (R3)
//!
//! Rules: GQ001-GQ007
//!
//! When a real tree-sitter tree is available the checker uses AST node matching;
//! it falls back to the proven line-based implementation for dummy/error trees.

pub use crate::domain::lint::graphql::GraphqlChecker;
