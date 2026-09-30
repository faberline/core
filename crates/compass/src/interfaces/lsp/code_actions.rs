use std::collections::HashMap;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use super::argus_server::ArgusServer;

impl ArgusServer {
    pub(super) async fn handle_code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let request_range = params.range;

        // Get the diagnostics of the latest analysis
        let Some(diagnostics) = self.session.diagnostics(uri.as_str()).await else {
            return Ok(None);
        };

        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        // Find diagnostics that overlap with the requested range
        for diag in &diagnostics {
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
        self.add_refactoring_actions(uri, &request_range, &mut actions)
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

    /// Add refactoring code actions based on the selected range.
    async fn add_refactoring_actions(
        &self,
        uri: &Url,
        range: &Range,
        actions: &mut Vec<CodeActionOrCommand>,
    ) {
        let proposals = self
            .session
            .refactorings(uri.as_str(), Self::from_lsp_range(range))
            .await;

        'proposals: for proposal in proposals {
            // Convert file edits to LSP workspace edits
            let mut changes = HashMap::new();

            for (file_path, edits) in proposal.edits {
                // Convert path to URI
                let Ok(file_uri) = Url::from_file_path(&file_path) else {
                    continue 'proposals;
                };

                let lsp_edits: Vec<TextEdit> = edits
                    .iter()
                    .map(|edit| TextEdit {
                        range: Self::to_lsp_range(&edit.range),
                        new_text: edit.new_text.clone(),
                    })
                    .collect();

                changes.insert(file_uri, lsp_edits);
            }

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: proposal.title,
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
            }));
        }
    }
}
