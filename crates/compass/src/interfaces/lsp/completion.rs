use tower_lsp::lsp_types::*;

use crate::application::editor::views::{CompletionCandidate, CompletionKind};

/// Convert a completion candidate to an LSP completion item
pub(super) fn to_completion_item(candidate: CompletionCandidate) -> CompletionItem {
    CompletionItem {
        label: candidate.label,
        kind: Some(to_completion_item_kind(candidate.kind)),
        detail: candidate.detail,
        documentation: candidate.documentation.map(Documentation::String),
        ..Default::default()
    }
}

/// Convert a completion kind to an LSP completion item kind
fn to_completion_item_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Function => CompletionItemKind::FUNCTION,
        CompletionKind::Class => CompletionItemKind::CLASS,
        CompletionKind::Variable => CompletionItemKind::VARIABLE,
        CompletionKind::Value => CompletionItemKind::VALUE,
        CompletionKind::Method => CompletionItemKind::METHOD,
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
    }
}
