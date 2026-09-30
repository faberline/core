use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::diagnostic::Diagnostic;
use crate::domain::check::lint_config::LintConfig;
use crate::infrastructure::analysis_cache::disk_cache::DiskCache;
use crate::lint::CheckerRegistry;
use crate::semantic::SymbolTable;
use crate::storage::resolve_cache_dir;
use crate::syntax::{MultiParser, ParsedFile};
use crate::type_inference::{SemanticModel, StubLoader};

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
    /// Type stubs
    #[allow(dead_code)]
    stubs: Arc<RwLock<StubLoader>>,
    /// Parser (not thread-safe, needs mutex)
    pub(super) parser: Arc<tokio::sync::Mutex<MultiParser>>,
    /// Persistent disk cache
    pub(super) disk_cache: Arc<DiskCache>,
}

impl RequestHandler {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let parser = MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

        let mut stubs = StubLoader::new();
        stubs.load_builtins();

        let cache_dir = resolve_cache_dir(&root)
            .unwrap_or_else(|_| root.join("cclab").join(".index").join("cache"));
        let disk_cache = Arc::new(DiskCache::new(cache_dir));

        Ok(Self {
            root,
            cache: Arc::new(RwLock::new(HashMap::new())),
            overrides: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(CheckerRegistry::new()),
            config: Arc::new(LintConfig::default()),
            stubs: Arc::new(RwLock::new(stubs)),
            parser: Arc::new(tokio::sync::Mutex::new(parser)),
            disk_cache,
        })
    }

    /// Create a handler for a specific scope (#1127).
    ///
    /// Uses per-scope cache directory and adds scope's search paths to stub loader.
    pub fn new_with_scope(
        root: PathBuf,
        scope_id: &str,
        project_root: &std::path::Path,
        extra_search_paths: &[PathBuf],
    ) -> Result<Self, String> {
        let parser = MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

        let mut stubs = StubLoader::new();
        stubs.load_builtins();
        for path in extra_search_paths {
            stubs.add_stub_path(path.clone());
        }

        let cache_dir = crate::storage::resolve_scope_cache_dir(project_root, scope_id)
            .unwrap_or_else(|_| {
                project_root
                    .join("cclab/.index/scopes")
                    .join(scope_id)
                    .join("cache")
            });
        let disk_cache = Arc::new(DiskCache::new(cache_dir));

        Ok(Self {
            root,
            cache: Arc::new(RwLock::new(HashMap::new())),
            overrides: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(CheckerRegistry::new()),
            config: Arc::new(LintConfig::default()),
            stubs: Arc::new(RwLock::new(stubs)),
            parser: Arc::new(tokio::sync::Mutex::new(parser)),
            disk_cache,
        })
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
