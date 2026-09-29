//! Cross-file rename refactoring
//!
//! Finds all references to a symbol (definitions, usages, imports) across
//! the project and generates `TextEdit`s to rename each occurrence.

pub use crate::domain::refactoring::rename::RenameEngine;
