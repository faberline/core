use crate::domain::diagnostic::model::{Diagnostic, DiagnosticSeverity};
use crate::domain::syntax::language::Language;

/// Result of checking a single file
#[derive(Debug)]
pub struct FileResult {
    pub path: std::path::PathBuf,
    pub language: Language,
    pub diagnostics: Vec<Diagnostic>,
}

impl FileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }
}
