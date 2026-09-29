use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;

// ============================================================================
// Refactoring Result
// ============================================================================

/// Result of a refactoring operation.
#[derive(Debug, Clone)]
pub struct RefactorResult {
    /// Edits to apply per file
    pub file_edits: HashMap<PathBuf, Vec<TextEdit>>,
    /// New files to create
    pub new_files: HashMap<PathBuf, String>,
    /// Files to delete
    pub deleted_files: Vec<PathBuf>,
    /// Import changes per file
    pub import_changes: HashMap<PathBuf, Vec<ImportChange>>,
    /// Diagnostics/warnings
    pub diagnostics: Vec<RefactorDiagnostic>,
}

/// A text edit to apply.
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// Span to replace
    pub span: Span,
    /// New text
    pub new_text: String,
}

/// A change to imports.
#[derive(Debug, Clone)]
pub enum ImportChange {
    /// Add an import
    Add { module: String, names: Vec<String> },
    /// Remove an import
    Remove { module: String, names: Vec<String> },
    /// Update an import
    Update {
        module: String,
        old_names: Vec<String>,
        new_names: Vec<String>,
    },
}

/// A diagnostic from refactoring.
#[derive(Debug, Clone)]
pub struct RefactorDiagnostic {
    /// Severity level
    pub level: DiagnosticLevel,
    /// Message
    pub message: String,
    /// Related file
    pub file: Option<PathBuf>,
    /// Related span
    pub span: Option<Span>,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
    Hint,
}

impl RefactorResult {
    /// Create an empty result.
    pub fn empty() -> Self {
        Self {
            file_edits: HashMap::new(),
            new_files: HashMap::new(),
            deleted_files: Vec::new(),
            import_changes: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Check if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.file_edits.is_empty() || !self.new_files.is_empty() || !self.deleted_files.is_empty()
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    /// Add a text edit for a file.
    pub fn add_edit(&mut self, file: PathBuf, edit: TextEdit) {
        self.file_edits.entry(file).or_default().push(edit);
    }

    /// Add a diagnostic.
    pub fn add_diagnostic(
        &mut self,
        level: DiagnosticLevel,
        message: impl Into<String>,
        file: Option<PathBuf>,
        span: Option<Span>,
    ) {
        self.diagnostics.push(RefactorDiagnostic {
            level,
            message: message.into(),
            file,
            span,
        });
    }
}
