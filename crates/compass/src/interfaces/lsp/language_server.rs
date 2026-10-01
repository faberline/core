use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;

use crate::syntax::Language;

use super::argus_server::{ArgusServer, Document};

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

        let Some(language) = Self::detect_language(&uri) else {
            return;
        };

        // Store document
        {
            let mut documents = self.documents.write().await;
            documents.insert(
                uri.clone(),
                Document {
                    content,
                    language,
                    version,
                },
            );
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
        {
            let mut documents = self.documents.write().await;
            if let Some(doc) = documents.get_mut(&uri) {
                doc.content = change.text;
                doc.version = version;
            }
        }

        // Analyze
        self.analyze_document(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        // Update content if provided
        if let Some(text) = params.text {
            let mut documents = self.documents.write().await;
            if let Some(doc) = documents.get_mut(&uri) {
                doc.content = text;
            }
        }

        // Re-analyze
        self.analyze_document(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        // Remove document and analysis
        {
            let mut documents = self.documents.write().await;
            documents.remove(&uri);
        }
        {
            let mut analyses = self.analyses.write().await;
            analyses.remove(&uri);
        }

        // Clear diagnostics
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Get document language
        let language = {
            let documents = self.documents.read().await;
            documents.get(uri).map(|d| d.language)
        };

        let Some(language) = language else {
            return Ok(None);
        };

        // Get analysis
        let analyses = self.analyses.read().await;
        let Some(analysis) = analyses.get(uri) else {
            return Ok(None);
        };

        // Find symbol at position
        let symbol = analysis
            .symbol_table
            .find_at_position(position.line, position.character);

        let Some(symbol) = symbol else {
            return Ok(None);
        };

        // Generate hover content
        let content = symbol.hover_content(language);

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some(Self::to_lsp_range(&symbol.location)),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Get analysis
        let analyses = self.analyses.read().await;
        let Some(analysis) = analyses.get(uri) else {
            return Ok(None);
        };

        // Find definition
        let symbol = analysis
            .symbol_table
            .find_definition_at(position.line, position.character);

        let Some(symbol) = symbol else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: Self::to_lsp_range(&symbol.location),
        })))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        // Get analysis
        let analyses = self.analyses.read().await;
        let Some(analysis) = analyses.get(uri) else {
            return Ok(None);
        };

        // Find references
        let references = analysis.symbol_table.find_references_at(
            position.line,
            position.character,
            include_declaration,
        );

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

        // Get document content
        let _content = {
            let documents = self.documents.read().await;
            documents.get(&uri).map(|d| d.content.clone())
        };

        let Some(_content) = _content else {
            return Ok(None);
        };

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

        // Get document content
        let doc_content = {
            let documents = self.documents.read().await;
            documents.get(uri).map(|d| (d.content.clone(), d.language))
        };

        let Some((content, language)) = doc_content else {
            return Ok(None);
        };

        // Only provide Python completions for now
        if language != Language::Python {
            return Ok(None);
        }

        // Get the line and figure out what we're completing
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;

        if line_idx >= lines.len() {
            return Ok(None);
        }

        let line = lines[line_idx];
        let col = position.character as usize;
        let prefix = &line[..col.min(line.len())];

        // Check if this is a dot completion
        let items = if prefix.ends_with('.') {
            // Get word before the dot
            self.complete_attribute(prefix).await
        } else if let Some(trigger) = params.context.and_then(|c| c.trigger_character) {
            if trigger == "." {
                self.complete_attribute(prefix).await
            } else {
                self.complete_identifiers(prefix).await
            }
        } else {
            self.complete_identifiers(prefix).await
        };

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }
}
