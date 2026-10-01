use std::collections::HashMap;
use std::path::PathBuf;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::type_inference::{
    RefactorKind, RefactorOptions, RefactorRequest, SignatureChanges, Span as ArgusSpan,
};

use super::argus_server::ArgusServer;

impl ArgusServer {
    pub(super) async fn handle_code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let request_range = params.range;

        // Get document content
        let content = {
            let documents = self.documents.read().await;
            documents.get(uri).map(|d| d.content.clone())
        };

        let Some(content) = content else {
            return Ok(None);
        };

        // Get analysis with diagnostics
        let analyses = self.analyses.read().await;
        let Some(analysis) = analyses.get(uri) else {
            return Ok(None);
        };

        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        // Find diagnostics that overlap with the requested range
        for diag in &analysis.diagnostics {
            let diag_range = Self::to_lsp_range(&diag.range);

            // Check if diagnostic overlaps with requested range
            if !Self::ranges_overlap(&diag_range, &request_range) {
                continue;
            }

            // Create code actions for each quick fix
            for fix in &diag.quick_fixes {
                let edits: Vec<TextEdit> = fix
                    .edits
                    .iter()
                    .map(|e| TextEdit {
                        range: Self::to_lsp_range(&e.range),
                        new_text: e.new_text.clone(),
                    })
                    .collect();

                let mut changes = HashMap::new();
                changes.insert(uri.clone(), edits);

                let action = CodeAction {
                    title: fix.title.clone(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![self.to_lsp_diagnostic(diag)]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    command: None,
                    is_preferred: Some(true),
                    disabled: None,
                    data: None,
                };

                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }

        // Add refactoring actions based on selection
        self.add_refactoring_actions(uri, &request_range, &content, &mut actions)
            .await;

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    /// Check if two ranges overlap
    fn ranges_overlap(a: &Range, b: &Range) -> bool {
        // a starts before b ends AND a ends after b starts
        (a.start.line < b.end.line
            || (a.start.line == b.end.line && a.start.character <= b.end.character))
            && (a.end.line > b.start.line
                || (a.end.line == b.start.line && a.end.character >= b.start.character))
    }

    /// Convert byte offset to LSP Position.
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

        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    /// Add refactoring code actions based on the selected range.
    async fn add_refactoring_actions(
        &self,
        uri: &Url,
        range: &Range,
        content: &str,
        actions: &mut Vec<CodeActionOrCommand>,
    ) {
        // Convert LSP range to Argus span
        let lines: Vec<&str> = content.lines().collect();
        let start_line = range.start.line as usize;
        let end_line = range.end.line as usize;

        if start_line >= lines.len() || end_line >= lines.len() {
            return;
        }

        // Calculate byte offsets for the selection
        let start_offset: usize = lines
            .iter()
            .take(start_line)
            .map(|l| l.len() + 1)
            .sum::<usize>()
            + range.start.character as usize;
        let end_offset: usize = lines
            .iter()
            .take(end_line)
            .map(|l| l.len() + 1)
            .sum::<usize>()
            + range.end.character as usize;

        let span = ArgusSpan::new(start_offset, end_offset);
        let file = PathBuf::from(uri.path());

        // Extract Variable - if something is selected
        if start_offset < end_offset {
            if let Some(action) = self
                .create_refactor_action(
                    uri,
                    content,
                    RefactorKind::ExtractVariable {
                        name: "extracted_var".to_string(),
                    },
                    span,
                    file.clone(),
                    "Extract to variable",
                )
                .await
            {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }

            // Extract Function
            if let Some(action) = self
                .create_refactor_action(
                    uri,
                    content,
                    RefactorKind::ExtractFunction {
                        name: "extracted_function".to_string(),
                    },
                    span,
                    file.clone(),
                    "Extract to function",
                )
                .await
            {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }

        // Rename Symbol - always available at cursor position
        if let Some(action) = self
            .create_refactor_action(
                uri,
                content,
                RefactorKind::Rename {
                    new_name: "new_name".to_string(),
                },
                span,
                file.clone(),
                "Rename symbol",
            )
            .await
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }

        // Inline Variable - if on a variable definition
        if let Some(action) = self
            .create_refactor_action(
                uri,
                content,
                RefactorKind::Inline,
                span,
                file.clone(),
                "Inline variable",
            )
            .await
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }

        // Change Signature - if on a function definition
        if let Some(action) = self
            .create_refactor_action(
                uri,
                content,
                RefactorKind::ChangeSignature {
                    changes: SignatureChanges::default(),
                },
                span,
                file,
                "Change function signature",
            )
            .await
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    /// Create a refactoring code action by executing the refactoring.
    async fn create_refactor_action(
        &self,
        _uri: &Url,
        content: &str,
        kind: RefactorKind,
        span: ArgusSpan,
        file: PathBuf,
        title: &str,
    ) -> Option<CodeAction> {
        let request = RefactorRequest {
            kind,
            file,
            span,
            options: RefactorOptions::default(),
        };

        // Execute refactoring
        let mut engine = self.refactoring_engine.write().await;
        let result = engine.execute(&request, content);

        // Check if refactoring succeeded
        if result.has_errors() {
            return None;
        }

        // Convert file edits to LSP workspace edits
        let mut changes = HashMap::new();

        for (file_path, edits) in result.file_edits {
            // Convert path to URI
            let file_uri = Url::from_file_path(&file_path).ok()?;

            // Convert edits to LSP text edits
            let lsp_edits: Vec<TextEdit> = edits
                .iter()
                .map(|edit| {
                    let start_pos = Self::offset_to_position(content, edit.span.start);
                    let end_pos = Self::offset_to_position(content, edit.span.end);

                    TextEdit {
                        range: Range {
                            start: start_pos,
                            end: end_pos,
                        },
                        new_text: edit.new_text.clone(),
                    }
                })
                .collect();

            changes.insert(file_uri, lsp_edits);
        }

        Some(CodeAction {
            title: title.to_string(),
            kind: Some(CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(false),
            disabled: None,
            data: None,
        })
    }
}
