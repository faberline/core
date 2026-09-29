use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;

// ============================================================================
// Refactoring Request
// ============================================================================

/// A request for a refactoring operation.
#[derive(Debug, Clone)]
pub struct RefactorRequest {
    /// Type of refactoring
    pub kind: RefactorKind,
    /// Target file
    pub file: PathBuf,
    /// Target span in source
    pub span: Span,
    /// Additional options
    pub options: RefactorOptions,
}

/// Type of refactoring operation.
#[derive(Debug, Clone)]
pub enum RefactorKind {
    /// Extract code into a new function
    ExtractFunction { name: String },
    /// Extract code into a new method
    ExtractMethod { name: String },
    /// Extract expression into a variable
    ExtractVariable { name: String },
    /// Rename a symbol
    Rename { new_name: String },
    /// Move a definition to another file
    MoveDefinition { target_file: PathBuf },
    /// Inline a symbol's definition
    Inline,
    /// Change function signature
    ChangeSignature { changes: SignatureChanges },
}

/// Options for refactoring operations.
#[derive(Debug, Clone, Default)]
pub struct RefactorOptions {
    /// Preview changes without applying
    pub preview_only: bool,
    /// Update imports automatically
    pub update_imports: bool,
    /// Add type annotations
    pub add_type_annotations: bool,
    /// Preserve formatting
    pub preserve_formatting: bool,
}

/// Changes to a function signature.
#[derive(Debug, Clone, Default)]
pub struct SignatureChanges {
    /// New parameters (name, type_annotation, default)
    pub new_params: Vec<(String, Option<String>, Option<String>)>,
    /// Reordered parameter indices
    pub param_order: Vec<usize>,
    /// Removed parameter indices
    pub removed_params: Vec<usize>,
    /// New return type annotation
    pub new_return_type: Option<String>,
}
