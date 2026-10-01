//! Diagnostic types (LSP-compatible)

pub use crate::domain::diagnostic::model::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, QuickFix, Range, TextEdit,
};
pub use crate::domain::diagnostic::rule_code::RuleCode;
