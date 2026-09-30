//! The editor session: the open documents, their latest analysis and the
//! engines that answer editor requests.

use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::{Mutex, RwLock};

use crate::application::editor::views::HoverView;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::diagnostic::model::{Diagnostic, Range};
use crate::domain::lint::registry::CheckerRegistry;
use crate::domain::modules::import::ModuleInfo;
use crate::domain::semantic::symbols::{SymbolTable, SymbolTableBuilder};
use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::stubs::bundled::bundled_stubs;
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::domain::syntax::source_parser::SourceParser;
use crate::domain::type_refactoring::engine::RefactoringEngine;

/// An open document.
#[derive(Debug)]
pub(super) struct Document {
    pub(super) path: PathBuf,
    pub(super) content: String,
    pub(super) language: Language,
    pub(super) version: i32,
}

/// The latest analysis of a document.
struct DocumentAnalysis {
    symbol_table: SymbolTable,
    /// Diagnostics with their quick fixes
    diagnostics: Vec<Diagnostic>,
}

/// The open documents of one editor, keyed by URI, and their analyses.
pub(crate) struct EditorSession {
    pub(super) documents: RwLock<HashMap<String, Document>>,
    analyses: RwLock<HashMap<String, DocumentAnalysis>>,
    registry: CheckerRegistry,
    config: LintConfig,
    parser: Mutex<Box<dyn SourceParser + Send + Sync>>,
    /// The bundled stub modules, for completions.
    pub(super) modules: HashMap<String, ModuleInfo>,
    search_engine: RwLock<SemanticSearchEngine>,
    pub(super) refactoring_engine: Mutex<RefactoringEngine>,
}

impl EditorSession {
    /// A session with no open documents, parsing with `parser` and
    /// refactoring with `refactoring_engine`.
    pub(crate) fn new(
        parser: Box<dyn SourceParser + Send + Sync>,
        refactoring_engine: RefactoringEngine,
    ) -> Self {
        Self {
            documents: RwLock::new(HashMap::new()),
            analyses: RwLock::new(HashMap::new()),
            registry: CheckerRegistry::new(),
            config: LintConfig::default(),
            parser: Mutex::new(parser),
            modules: bundled_stubs(),
            search_engine: RwLock::new(SemanticSearchEngine::new()),
            refactoring_engine: Mutex::new(refactoring_engine),
        }
    }

    /// Open a document at `path`. Returns false, and keeps nothing, when the
    /// path's language is not one compass analyses.
    pub(crate) async fn open(
        &self,
        uri: &str,
        path: PathBuf,
        content: String,
        version: i32,
    ) -> bool {
        let Some(language) = Language::from_path(&path) else {
            return false;
        };
        self.documents.write().await.insert(
            uri.to_string(),
            Document {
                path,
                content,
                language,
                version,
            },
        );
        true
    }

    /// Replace the content of an open document (full sync).
    pub(crate) async fn change(&self, uri: &str, content: String, version: i32) {
        if let Some(doc) = self.documents.write().await.get_mut(uri) {
            doc.content = content;
            doc.version = version;
        }
    }

    /// Record a save; `content` is the saved text when the client sent it.
    pub(crate) async fn save(&self, uri: &str, content: Option<String>) {
        if let Some(content) = content {
            if let Some(doc) = self.documents.write().await.get_mut(uri) {
                doc.content = content;
            }
        }
    }

    /// Forget a document and its analysis.
    pub(crate) async fn close(&self, uri: &str) {
        self.documents.write().await.remove(uri);
        self.analyses.write().await.remove(uri);
    }

    /// Whether a document is open.
    pub(crate) async fn is_open(&self, uri: &str) -> bool {
        self.documents.read().await.contains_key(uri)
    }

    /// Analyse a document: lint it, rebuild its symbol table and, for
    /// Python, index it for semantic search. Returns the diagnostics to
    /// publish, or `None` when the document is not open or does not parse.
    pub(crate) async fn analyse(&self, uri: &str) -> Option<Vec<Diagnostic>> {
        let (path, content, language) = {
            let documents = self.documents.read().await;
            let doc = documents.get(uri)?;
            (doc.path.clone(), doc.content.clone(), doc.language)
        };

        let parsed = self.parser.lock().await.parse(&content, language)?;

        let diagnostics = match self.registry.get(language) {
            Some(checker) => checker.check(&parsed, &self.config),
            None => Vec::new(),
        };
        let symbol_table = build_symbol_table(&parsed, language);

        // Index symbols, build call graph, and extract docstrings for semantic search
        if language == Language::Python {
            let mut search_engine = self.search_engine.write().await;
            search_engine.index_symbol_table(path.clone(), &symbol_table);
            search_engine.build_call_graph_parsed(path, &parsed);
            let docstrings = search_engine.extract_docstrings_parsed(&parsed);
            search_engine.update_docstrings(docstrings);
        }

        self.analyses.write().await.insert(
            uri.to_string(),
            DocumentAnalysis {
                symbol_table,
                diagnostics: diagnostics.clone(),
            },
        );
        Some(diagnostics)
    }

    /// Hover text for the symbol at a position.
    pub(crate) async fn hover(&self, uri: &str, line: u32, character: u32) -> Option<HoverView> {
        let language = self.documents.read().await.get(uri)?.language;
        let analyses = self.analyses.read().await;
        let symbol = analyses
            .get(uri)?
            .symbol_table
            .find_at_position(line, character)?;
        Some(HoverView {
            markdown: symbol.hover_content(language),
            range: symbol.location,
        })
    }

    /// The range of the definition of the symbol at a position.
    pub(crate) async fn definition(&self, uri: &str, line: u32, character: u32) -> Option<Range> {
        let analyses = self.analyses.read().await;
        let symbol = analyses
            .get(uri)?
            .symbol_table
            .find_definition_at(line, character)?;
        Some(symbol.location)
    }

    /// The ranges of the references to the symbol at a position.
    pub(crate) async fn references(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Vec<Range> {
        let analyses = self.analyses.read().await;
        match analyses.get(uri) {
            Some(analysis) => {
                analysis
                    .symbol_table
                    .find_references_at(line, character, include_declaration)
            }
            None => Vec::new(),
        }
    }

    /// The diagnostics of the latest analysis of an open document.
    pub(crate) async fn diagnostics(&self, uri: &str) -> Option<Vec<Diagnostic>> {
        if !self.is_open(uri).await {
            return None;
        }
        let analyses = self.analyses.read().await;
        analyses
            .get(uri)
            .map(|analysis| analysis.diagnostics.clone())
    }
}

/// Build the symbol table of a parsed file.
fn build_symbol_table(parsed: &ParsedFile, language: Language) -> SymbolTable {
    let builder = SymbolTableBuilder::new();
    match language {
        Language::Python => builder.build_python(parsed),
        Language::Rust => builder.build_rust(parsed),
        Language::TypeScript => builder.build_typescript(parsed),
        Language::JavaScript => builder.build_javascript(parsed),
        Language::Html => builder.build_html(parsed),
        Language::Css => builder.build_css(parsed),
        Language::Go => builder.build_go(parsed),
        Language::Dockerfile => builder.build_dockerfile(parsed),
        Language::Hcl => builder.build_terraform(parsed),
        Language::Yaml => builder.build_kubernetes(parsed),
        Language::Markdown | Language::Mdx => builder.build_markdown(parsed),
        Language::Mermaid => builder.build_mermaid(parsed),
        Language::Toml => builder.build_toml(parsed),
        Language::Sql => builder.build_sql(parsed),
        Language::Proto => builder.build_proto(parsed),
        Language::GraphQL => builder.build_graphql(parsed),
    }
}
