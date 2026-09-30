//! Mutable AST with persistent data structures (Sprint 2 - Track 2)
//!
//! Provides a mutable AST implementation for efficient code transformations:
//! - Copy-on-write node modifications
//! - Persistent immutable snapshots
//! - Efficient tree diffing
//! - Undo/redo support

pub use crate::domain::ast_editing::mutable_ast::{
    AstEdit, MutableAst, MutableNode, NodeId, NodeMetadata, NodeRef, Span, TreeDiff,
};
