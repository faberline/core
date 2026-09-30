use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;

use super::argus_server::ArgusServer;
use super::completion::to_completion_item;

#[tower_lsp::async_trait]
impl LanguageServer for ArgusServer {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                // Semantic capabilities
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                // Completion
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                // Code actions (quick fixes + refactoring)
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::REFACTOR_EXTRACT,
                            CodeActionKind::REFACTOR_INLINE,
                            CodeActionKind::REFACTOR_REWRITE,
                        ]),
                        ..Default::default()
                    },
                )),
                // Execute command for refactoring with user input
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "cclab_lens.refactor.extractVariable".to_string(),
                        "cclab_lens.refactor.extractFunction".to_string(),
                        "cclab_lens.refactor.rename".to_string(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cclab_lens".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Argus LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;

        // Store document
        let path = Self::document_path(&uri);
        if !self
            .session
            .open(uri.as_str(), path, content, version)
            .await
        {
            return;
        }

        // Analyze
        self.analyze_document(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // Get full content (we use FULL sync)
        let Some(change) = params.content_changes.into_iter().next() else {
            return;
        };

        // Update document
        self.session
            .change(uri.as_str(), change.text, version)
            .await;

        // Analyze
        self.analyze_document(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        // Update content if provided
        self.session.save(uri.as_str(), params.text).await;

        // Re-analyze
        self.analyze_document(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        // Remove document and analysis
        self.session.close(uri.as_str()).await;

        // Clear diagnostics
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Find symbol at position
        let Some(hover) = self
            .session
            .hover(uri.as_str(), position.line, position.character)
            .await
        else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover.markdown,
            }),
            range: Some(Self::to_lsp_range(&hover.range)),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Find definition
        let Some(range) = self
            .session
            .definition(uri.as_str(), position.line, position.character)
            .await
        else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: Self::to_lsp_range(&range),
        })))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        // Find references
        let references = self
            .session
            .references(
                uri.as_str(),
                position.line,
                position.character,
                include_declaration,
            )
            .await;

        if references.is_empty() {
            return Ok(None);
        }

        let locations: Vec<Location> = references
            .into_iter()
            .map(|r| Location {
                uri: uri.clone(),
                range: Self::to_lsp_range(&r),
            })
            .collect();

        Ok(Some(locations))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        self.handle_code_action(params).await
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        // Execute refactoring commands with user-provided parameters
        let command = &params.command;

        // Parse arguments from params.arguments
        // Expected format: [{"uri": "file://...", "range": {...}, "name": "..."}]
        if params.arguments.is_empty() {
            return Ok(None);
        }

        let arg = &params.arguments[0];

        // Extract parameters (simplified - real implementation would use proper JSON deserialization)
        let uri_str = arg.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let name = arg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("refactored");

        let uri = match Url::parse(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        // The document must be open
        if !self.session.is_open(uri.as_str()).await {
            return Ok(None);
        }

        // Execute the appropriate refactoring based on command
        // This would apply the refactoring and return workspace edits
        // For now, just log success
        self.client
            .log_message(
                MessageType::INFO,
                format!("Executing command: {} with name: {}", command, name),
            )
            .await;

        Ok(Some(Value::Null))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let dot_trigger = params
            .context
            .and_then(|c| c.trigger_character)
            .is_some_and(|trigger| trigger == ".");

        let Some(candidates) = self
            .session
            .completions(uri.as_str(), position.line, position.character, dot_trigger)
            .await
        else {
            return Ok(None);
        };

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(
                candidates.into_iter().map(to_completion_item).collect(),
            )))
        }
    }
}
