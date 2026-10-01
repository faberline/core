//! Refactoring operations (Sprint 3 - Track 1)
//!
//! Provides type-aware refactoring operations:
//! - Extract function/method/variable
//! - Rename symbol (cross-file)
//! - Move definition
//! - Inline symbol
//! - Change signature

pub use crate::domain::type_refactoring::engine::RefactoringEngine;
pub use crate::domain::type_refactoring::request::{
    RefactorKind, RefactorOptions, RefactorRequest, SignatureChanges,
};
pub use crate::domain::type_refactoring::result::{
    DiagnosticLevel, ImportChange, RefactorDiagnostic, RefactorResult, TextEdit,
};
