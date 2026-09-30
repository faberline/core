//! What the editor use cases hand to the language server.
//!
//! Positions, ranges, diagnostics and text edits are the domain's
//! LSP-shaped value objects; the rest are the views below.

use std::path::PathBuf;

use crate::domain::diagnostic::model::{Range, TextEdit};

/// The kind of a completion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionKind {
    Function,
    Class,
    Variable,
    Value,
    Method,
    Keyword,
}

/// One completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionCandidate {
    pub(crate) label: String,
    pub(crate) kind: CompletionKind,
    pub(crate) detail: Option<String>,
    pub(crate) documentation: Option<String>,
}

/// Hover text (Markdown) for the symbol under the cursor, and the symbol's
/// range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HoverView {
    pub(crate) markdown: String,
    pub(crate) range: Range,
}

/// A refactoring the editor can apply: its title and the edits per file.
#[derive(Debug, Clone)]
pub(crate) struct RefactorProposal {
    pub(crate) title: String,
    pub(crate) edits: Vec<(PathBuf, Vec<TextEdit>)>,
}
