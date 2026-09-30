//! Refactoring proposals for a selection: each candidate refactoring is run
//! on the document, and those that succeed are offered.

use std::path::PathBuf;

use crate::application::editor::session::EditorSession;
use crate::application::editor::views::RefactorProposal;
use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::diagnostic::model::{Position, Range, TextEdit};
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::{
    RefactorKind, RefactorOptions, RefactorRequest, SignatureChanges,
};

impl EditorSession {
    /// The refactorings that apply to `selection` in an open document:
    /// extract to variable and to function (non-empty selections only),
    /// rename, inline and change signature. Refactorings that report errors
    /// are left out.
    pub(crate) async fn refactorings(&self, uri: &str, selection: Range) -> Vec<RefactorProposal> {
        let Some((file, content)) = ({
            let documents = self.documents.read().await;
            documents
                .get(uri)
                .map(|d| (d.path.clone(), d.content.clone()))
        }) else {
            return Vec::new();
        };

        let Some((start_offset, end_offset)) = selection_offsets(&content, selection) else {
            return Vec::new();
        };
        let span = Span::new(start_offset, end_offset);

        let mut candidates = Vec::new();
        if start_offset < end_offset {
            candidates.push((
                RefactorKind::ExtractVariable {
                    name: "extracted_var".to_string(),
                },
                "Extract to variable",
            ));
            candidates.push((
                RefactorKind::ExtractFunction {
                    name: "extracted_function".to_string(),
                },
                "Extract to function",
            ));
        }
        // Rename Symbol - always available at cursor position
        candidates.push((
            RefactorKind::Rename {
                new_name: "new_name".to_string(),
            },
            "Rename symbol",
        ));
        candidates.push((RefactorKind::Inline, "Inline variable"));
        candidates.push((
            RefactorKind::ChangeSignature {
                changes: SignatureChanges::default(),
            },
            "Change function signature",
        ));

        let mut engine = self.refactoring_engine.lock().await;
        candidates
            .into_iter()
            .filter_map(|(kind, title)| {
                propose(&mut engine, &content, kind, span, file.clone(), title)
            })
            .collect()
    }
}

/// Byte offsets of a selection, or `None` when it ends past the last line.
fn selection_offsets(content: &str, selection: Range) -> Option<(usize, usize)> {
    let lines: Vec<&str> = content.lines().collect();
    let start_line = selection.start.line as usize;
    let end_line = selection.end.line as usize;

    if start_line >= lines.len() || end_line >= lines.len() {
        return None;
    }

    let line_start = |line: usize| -> usize { lines.iter().take(line).map(|l| l.len() + 1).sum() };
    Some((
        line_start(start_line) + selection.start.character as usize,
        line_start(end_line) + selection.end.character as usize,
    ))
}

/// Run one refactoring and turn its edits into editor text edits.
fn propose(
    engine: &mut RefactoringEngine,
    content: &str,
    kind: RefactorKind,
    span: Span,
    file: PathBuf,
    title: &str,
) -> Option<RefactorProposal> {
    let request = RefactorRequest {
        kind,
        file,
        span,
        options: RefactorOptions::default(),
    };

    let result = engine.execute(&request, content);
    if result.has_errors() {
        return None;
    }

    let edits = result
        .file_edits
        .into_iter()
        .map(|(path, edits)| {
            let edits = edits
                .iter()
                .map(|edit| TextEdit {
                    range: Range::new(
                        offset_to_position(content, edit.span.start),
                        offset_to_position(content, edit.span.end),
                    ),
                    new_text: edit.new_text.clone(),
                })
                .collect();
            (path, edits)
        })
        .collect();

    Some(RefactorProposal {
        title: title.to_string(),
        edits,
    })
}

/// Convert a byte offset to a position.
fn offset_to_position(content: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut character = 0;
    let mut current_offset = 0;

    for ch in content.chars() {
        if current_offset >= offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }

        current_offset += ch.len_utf8();
    }

    Position::new(line, character)
}
