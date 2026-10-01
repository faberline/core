use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::diagnostic::Diagnostic;
use crate::domain::analysis_cache::AnalysisCache;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::syntax::source_parser::SourceParser;
use crate::lint::CheckerRegistry;
use crate::semantic::SymbolTable;
use crate::syntax::ParsedFile;
use crate::type_inference::SemanticModel;

/// Cached analysis for a file
pub(super) struct FileAnalysis {
    #[allow(dead_code)]
    pub(super) parsed: ParsedFile,
    pub(super) symbol_table: SymbolTable,
    pub(super) semantic_model: SemanticModel,
    pub(super) diagnostics: Vec<Diagnostic>,
    #[allow(dead_code)]
    pub(super) source: String,
    /// Unix timestamp (seconds) when this file's diagnostics were last updated.
    pub(super) last_updated_secs: u64,
}

/// Request handler with caching
pub struct RequestHandler {
    /// Root directory being analyzed
    pub(super) root: PathBuf,
    /// File cache: path -> analysis
    pub(super) cache: Arc<RwLock<HashMap<PathBuf, FileAnalysis>>>,
    /// In-memory document overrides (for unsaved LSP changes)
    overrides: Arc<RwLock<HashMap<PathBuf, String>>>,
    /// Checker registry
    pub(super) registry: Arc<CheckerRegistry>,
    /// Lint configuration
    pub(super) config: Arc<LintConfig>,
    /// Parser (not thread-safe, needs mutex)
    pub(super) parser: Arc<tokio::sync::Mutex<Box<dyn SourceParser + Send>>>,
    /// Persistent analysis cache
    pub(super) disk_cache: Arc<dyn AnalysisCache>,
}

impl RequestHandler {
    /// A handler for `root` that parses with `parser` and keeps analysis
    /// across runs in `disk_cache`, with the default lint configuration.
    ///
    /// `RequestHandler::new` and `RequestHandler::new_with_scope` (in the
    /// composition root, src/app) build it with the tree-sitter parser and
    /// the on-disk cache.
    pub(crate) fn with_ports(
        root: PathBuf,
        parser: Box<dyn SourceParser + Send>,
        disk_cache: Arc<dyn AnalysisCache>,
    ) -> Self {
        Self {
            root,
            cache: Arc::new(RwLock::new(HashMap::new())),
            overrides: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(CheckerRegistry::new()),
            config: Arc::new(LintConfig::default()),
            parser: Arc::new(tokio::sync::Mutex::new(parser)),
            disk_cache,
        }
    }

    /// Set an in-memory document override (for unsaved LSP changes)
    pub async fn set_document_override(&self, path: impl AsRef<Path>, content: String) {
        let path = path.as_ref().to_path_buf();
        let mut overrides = self.overrides.write().await;
        overrides.insert(path.clone(), content);
        // Invalidate cache for this file since content changed
        let mut cache = self.cache.write().await;
        cache.remove(&path);
    }

    /// Remove an in-memory document override
    pub async fn remove_document_override(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        let mut overrides = self.overrides.write().await;
        overrides.remove(&path);
        // Invalidate cache for this file
        let mut cache = self.cache.write().await;
        cache.remove(&path);
    }

    /// Get document content, preferring overrides over disk
    pub async fn get_document_content(&self, path: &Path) -> Result<String, String> {
        // Check for override first
        {
            let overrides = self.overrides.read().await;
            if let Some(content) = overrides.get(path) {
                return Ok(content.clone());
            }
        }
        // Fall back to disk
        std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod tests;
