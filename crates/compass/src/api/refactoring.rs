//! Refactoring engine with pluggable operation strategies
//!
//! Dispatches refactoring requests to the appropriate engine based on
//! the `RefactorKind`. Each engine implements the `RefactoringOp` trait.

mod extract;
mod extract_helpers;
mod inline;
mod move_def;
mod rename;
mod signature;
mod signature_helpers;

pub use extract::ExtractEngine;
pub use inline::InlineEngine;
pub use move_def::MoveDefEngine;
pub use rename::RenameEngine;
pub use signature::SignatureEngine;

pub use crate::domain::refactoring::engine::{
    FileContext, ProjectContext, RefactoringOp, RefactoringRegistry,
};
