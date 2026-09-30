//! `RequestHandler::new` and `RequestHandler::new_with_scope`: the daemon's
//! request handler wired to the tree-sitter parser and the on-disk analysis
//! cache under the project's index directory.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::analysis::request_handler::RequestHandler;
use crate::infrastructure::analysis_cache::disk_cache::DiskCache;
use crate::infrastructure::index_storage::paths::{resolve_cache_dir, resolve_scope_cache_dir};
use crate::infrastructure::syntax::multi_parser::MultiParser;

impl RequestHandler {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let parser = MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

        let cache_dir = resolve_cache_dir(&root)
            .unwrap_or_else(|_| root.join("cclab").join(".index").join("cache"));
        let disk_cache = Arc::new(DiskCache::new(cache_dir));

        Ok(Self::with_ports(root, Box::new(parser), disk_cache))
    }

    /// Create a handler for a specific scope (#1127).
    ///
    /// Uses the per-scope cache directory. `extra_search_paths` is accepted
    /// for compatibility and unused: it fed a stub loader nothing consulted.
    pub fn new_with_scope(
        root: PathBuf,
        scope_id: &str,
        project_root: &Path,
        extra_search_paths: &[PathBuf],
    ) -> Result<Self, String> {
        let _ = extra_search_paths;
        let parser = MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

        let cache_dir = resolve_scope_cache_dir(project_root, scope_id).unwrap_or_else(|_| {
            project_root
                .join("cclab/.index/scopes")
                .join(scope_id)
                .join("cache")
        });
        let disk_cache = Arc::new(DiskCache::new(cache_dir));

        Ok(Self::with_ports(root, Box::new(parser), disk_cache))
    }
}
