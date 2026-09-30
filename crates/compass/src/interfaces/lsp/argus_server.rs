use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use crate::diagnostic::{Diagnostic as ArgusDiagnostic, DiagnosticSeverity as ArgusSeverity};
use crate::domain::check::lint_config::LintConfig;
use crate::lint::CheckerRegistry;
use crate::semantic::{SymbolTable, SymbolTableBuilder};
use crate::syntax::Language;
use crate::syntax::{MultiParser, ParsedFile};
use crate::type_inference::{RefactoringEngine, SemanticSearchEngine, StubLoader};

/// Document state tracked by the server
#[derive(Debug)]
pub(super) struct Document {
    pub(super) content: String,
    pub(super) language: Language,
    pub(super) version: i32,
}

/// Cached analysis for a document
pub(super) struct DocumentAnalysis {
    pub(super) symbol_table: SymbolTable,
    /// Diagnostics with their quick fixes
    pub(super) diagnostics: Vec<ArgusDiagnostic>,
}

/// Argus Language Server
pub struct ArgusServer {
    pub(super) client: Client,
    pub(super) documents: Arc<RwLock<HashMap<Url, Document>>>,
    pub(super) analyses: Arc<RwLock<HashMap<Url, DocumentAnalysis>>>,
    registry: Arc<CheckerRegistry>,
    config: Arc<LintConfig>,
    pub(super) stubs: Arc<RwLock<StubLoader>>,
    search_engine: Arc<RwLock<SemanticSearchEngine>>,
    pub(super) refactoring_engine: Arc<RwLock<RefactoringEngine>>,
}

impl ArgusServer {
    /// Create a new Argus server
    pub fn new(client: Client) -> Self {
        let mut stubs = StubLoader::new();
        stubs.load_builtins();

        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            analyses: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(CheckerRegistry::new()),
            config: Arc::new(LintConfig::default()),
            stubs: Arc::new(RwLock::new(stubs)),
            search_engine: Arc::new(RwLock::new(SemanticSearchEngine::new())),
            refactoring_engine: Arc::new(RwLock::new(RefactoringEngine::new())),
        }
    }

    /// Analyze a document and publish diagnostics
    pub(super) async fn analyze_document(&self, uri: &Url) {
        let documents = self.documents.read().await;
        let Some(doc) = documents.get(uri) else {
            return;
        };

        let content = doc.content.clone();
        let language = doc.language;
        drop(documents);

        // Parse
        let mut parser = match MultiParser::new() {
            Ok(p) => p,
            Err(_) => return,
        };

        let parsed = match parser.parse(&content, language) {
            Some(p) => p,
            None => return,
        };

        // Run linting
        let diagnostics = self.run_lint(&parsed, language);

        // Build symbol table and store diagnostics
        let symbol_table = match language {
            Language::Python => SymbolTableBuilder::new().build_python(&parsed),
            Language::Rust => SymbolTableBuilder::new().build_rust(&parsed),
            Language::TypeScript => SymbolTableBuilder::new().build_typescript(&parsed),
            Language::JavaScript => SymbolTableBuilder::new().build_javascript(&parsed),
            Language::Html => SymbolTableBuilder::new().build_html(&parsed),
            Language::Css => SymbolTableBuilder::new().build_css(&parsed),
            Language::Go => SymbolTableBuilder::new().build_go(&parsed),
            Language::Dockerfile => SymbolTableBuilder::new().build_dockerfile(&parsed),
            Language::Hcl => SymbolTableBuilder::new().build_terraform(&parsed),
            Language::Yaml => SymbolTableBuilder::new().build_kubernetes(&parsed),
            Language::Markdown | Language::Mdx => SymbolTableBuilder::new().build_markdown(&parsed),
            Language::Mermaid => SymbolTableBuilder::new().build_mermaid(&parsed),
            Language::Toml => SymbolTableBuilder::new().build_toml(&parsed),
            Language::Sql => SymbolTableBuilder::new().build_sql(&parsed),
            Language::Proto => SymbolTableBuilder::new().build_proto(&parsed),
            Language::GraphQL => SymbolTableBuilder::new().build_graphql(&parsed),
        };

        // Index symbols, build call graph, and extract docstrings for semantic search
        if language == Language::Python {
            let file_path = PathBuf::from(uri.path());
            let mut search_engine = self.search_engine.write().await;
            search_engine.index_symbol_table(file_path.clone(), &symbol_table);

            // Build call graph from AST
            if let Err(e) = search_engine.build_call_graph(file_path.clone(), &content, language) {
                eprintln!("Failed to build call graph: {}", e);
            }

            // Extract and index docstrings
            if let Ok(docstrings) = search_engine.extract_docstrings(&content, language) {
                search_engine.update_docstrings(docstrings);
            }
        }

        // Store analysis with diagnostics for code actions
        {
            let mut analyses = self.analyses.write().await;
            analyses.insert(
                uri.clone(),
                DocumentAnalysis {
                    symbol_table,
                    diagnostics: diagnostics.clone(),
                },
            );
        }

        // Convert to LSP diagnostics
        let lsp_diagnostics: Vec<tower_lsp::lsp_types::Diagnostic> = diagnostics
            .into_iter()
            .map(|d| self.to_lsp_diagnostic(&d))
            .collect();

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
            .await;
    }

    /// Run linting on parsed file
    fn run_lint(&self, parsed: &ParsedFile, language: Language) -> Vec<ArgusDiagnostic> {
        let checker = match self.registry.get(language) {
            Some(c) => c,
            None => return Vec::new(),
        };

        checker.check(parsed, &self.config)
    }

    /// Convert Argus diagnostic to LSP diagnostic
    pub(super) fn to_lsp_diagnostic(
        &self,
        diag: &ArgusDiagnostic,
    ) -> tower_lsp::lsp_types::Diagnostic {
        tower_lsp::lsp_types::Diagnostic {
            range: Range {
                start: Position {
                    line: diag.range.start.line,
                    character: diag.range.start.character,
                },
                end: Position {
                    line: diag.range.end.line,
                    character: diag.range.end.character,
                },
            },
            severity: Some(self.to_lsp_severity(diag.severity)),
            code: Some(NumberOrString::String(diag.code.clone())),
            code_description: None,
            source: Some("cclab_lens".to_string()),
            message: diag.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Convert Argus severity to LSP severity
    fn to_lsp_severity(&self, severity: ArgusSeverity) -> DiagnosticSeverity {
        match severity {
            ArgusSeverity::Error => DiagnosticSeverity::ERROR,
            ArgusSeverity::Warning => DiagnosticSeverity::WARNING,
            ArgusSeverity::Information => DiagnosticSeverity::INFORMATION,
            ArgusSeverity::Hint => DiagnosticSeverity::HINT,
        }
    }

    /// Detect language from URI
    pub(super) fn detect_language(uri: &Url) -> Option<Language> {
        let path = PathBuf::from(uri.path());
        MultiParser::detect_language(&path)
    }

    /// Convert Argus Range to LSP Range
    pub(super) fn to_lsp_range(range: &crate::diagnostic::Range) -> Range {
        Range {
            start: Position {
                line: range.start.line,
                character: range.start.character,
            },
            end: Position {
                line: range.end.line,
                character: range.end.character,
            },
        }
    }
}
