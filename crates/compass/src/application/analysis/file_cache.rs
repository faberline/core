use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::daemon::protocol::*;
use crate::diagnostic::Diagnostic;
use crate::semantic::{SymbolTable, SymbolTableBuilder};
use crate::syntax::{Language, MultiParser};
use crate::type_inference::{build_semantic_model, ContentHash, SemanticModel};

use super::request_handler::{FileAnalysis, RequestHandler};
use super::timestamp::current_unix_secs;

impl RequestHandler {
    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Resolve a path relative to root
    pub(super) fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.root.join(p)
        }
    }

    /// Collect all analyzable files in a directory
    pub(super) fn collect_files(&self, dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip excluded patterns
                if self.config.is_excluded(&path) {
                    continue;
                }

                if path.is_dir() {
                    files.extend(self.collect_files(&path));
                } else if path.is_file() {
                    if MultiParser::detect_language(&path).is_some() {
                        files.push(path);
                    }
                }
            }
        }

        files
    }

    /// Ensure a file is analyzed and cached.
    ///
    /// On in-memory miss, tries the disk cache before doing a full parse.
    pub(super) async fn ensure_analyzed(&self, path: &Path) -> Result<(), RpcError> {
        // Fast path: already in memory
        {
            let cache = self.cache.read().await;
            if cache.contains_key(path) {
                return Ok(());
            }
        }

        // Try disk cache
        if self.try_restore_from_disk(path).await {
            return Ok(());
        }

        // Full analysis
        self.check_file(path).await;
        Ok(())
    }

    /// Attempt to restore a file's analysis from disk cache.
    ///
    /// Returns `true` if the disk cache had a fresh entry and the
    /// in-memory cache was populated.
    async fn try_restore_from_disk(&self, path: &Path) -> bool {
        let source = match self.get_document_content(path).await {
            Ok(s) => s,
            Err(_) => return false,
        };

        let content_hash = ContentHash::from_content(&source);
        let persisted = match self.disk_cache.load(path, content_hash.0).await {
            Some(p) => p,
            None => return false,
        };

        // Re-parse to get Tree + SymbolTable (~1ms)
        let language = match MultiParser::detect_language(path) {
            Some(l) => l,
            None => return false,
        };

        let parsed = {
            let mut parser = self.parser.lock().await;
            match parser.parse(&source, language) {
                Some(p) => p,
                None => return false,
            }
        };

        let symbol_table = match language {
            Language::Python => SymbolTableBuilder::new().build_python(&parsed),
            Language::Rust => SymbolTableBuilder::new().build_rust(&parsed),
            Language::TypeScript => SymbolTableBuilder::new().build_typescript(&parsed),
            Language::JavaScript => SymbolTableBuilder::new().build_javascript(&parsed),
            Language::Go => SymbolTableBuilder::new().build_go(&parsed),
            Language::Toml => SymbolTableBuilder::new().build_toml(&parsed),
            Language::Sql => SymbolTableBuilder::new().build_sql(&parsed),
            Language::Proto => SymbolTableBuilder::new().build_proto(&parsed),
            Language::GraphQL => SymbolTableBuilder::new().build_graphql(&parsed),
            _ => SymbolTable::default(),
        };

        let mut cache = self.cache.write().await;
        cache.insert(
            path.to_path_buf(),
            FileAnalysis {
                parsed,
                symbol_table,
                semantic_model: persisted.semantic_model,
                diagnostics: persisted.diagnostics,
                source,
                last_updated_secs: current_unix_secs(),
            },
        );
        true
    }

    /// Check a single file and cache the results
    pub(super) async fn check_file(&self, path: &Path) -> Option<Vec<DiagnosticInfo>> {
        let language = MultiParser::detect_language(path)?;

        if !self.config.is_language_enabled(language) {
            return None;
        }

        let source = self.get_document_content(path).await.ok()?;

        // Parse
        let parsed = {
            let mut parser = self.parser.lock().await;
            parser.parse(&source, language)?
        };

        // Run linting
        let checker = self.registry.get(language)?;
        let diagnostics = checker.check(&parsed, &self.config);

        // Build symbol table
        let symbol_table = match language {
            Language::Python => SymbolTableBuilder::new().build_python(&parsed),
            Language::Rust => SymbolTableBuilder::new().build_rust(&parsed),
            Language::TypeScript => SymbolTableBuilder::new().build_typescript(&parsed),
            Language::JavaScript => SymbolTableBuilder::new().build_javascript(&parsed),
            Language::Go => SymbolTableBuilder::new().build_go(&parsed),
            Language::Toml => SymbolTableBuilder::new().build_toml(&parsed),
            Language::Sql => SymbolTableBuilder::new().build_sql(&parsed),
            Language::Proto => SymbolTableBuilder::new().build_proto(&parsed),
            Language::GraphQL => SymbolTableBuilder::new().build_graphql(&parsed),
            _ => SymbolTable::default(),
        };

        // Build semantic model for type analysis
        let semantic_model = if language == Language::Python {
            build_semantic_model(&parsed, &source, path.to_path_buf())
        } else {
            SemanticModel::new()
        };

        let diag_infos = self.convert_diagnostics(path, &diagnostics);

        // Write to disk cache in background
        let disk_cache = Arc::clone(&self.disk_cache);
        let disk_path = path.to_path_buf();
        let content_hash = ContentHash::from_content(&source);
        let disk_model = semantic_model.clone();
        let disk_diags = diagnostics.clone();
        tokio::spawn(async move {
            disk_cache
                .store(&disk_path, content_hash.0, &disk_model, &disk_diags)
                .await;
        });

        // Cache the analysis in memory
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                path.to_path_buf(),
                FileAnalysis {
                    parsed,
                    symbol_table,
                    semantic_model,
                    diagnostics,
                    source: source.clone(),
                    last_updated_secs: current_unix_secs(),
                },
            );
        }

        Some(diag_infos)
    }

    /// Convert diagnostics to protocol format
    pub(super) fn convert_diagnostics(
        &self,
        path: &Path,
        diagnostics: &[Diagnostic],
    ) -> Vec<DiagnosticInfo> {
        diagnostics
            .iter()
            .map(|d| DiagnosticInfo {
                file: path.to_string_lossy().to_string(),
                line: d.range.start.line,
                column: d.range.start.character,
                end_line: d.range.end.line,
                end_column: d.range.end.character,
                severity: format!("{:?}", d.severity).to_lowercase(),
                code: d.code.clone(),
                message: d.message.clone(),
            })
            .collect()
    }
}
