use std::path::Path;

use super::request_handler::RequestHandler;

impl RequestHandler {
    // =========================================================================
    // Public methods for background analysis
    // =========================================================================

    /// Proactively index all analyzable files in a directory
    pub async fn index_directory(&self, dir: &Path) -> usize {
        let files = self.collect_files(dir);
        let mut indexed = 0;
        for file in files {
            if self.check_file(&file).await.is_some() {
                indexed += 1;
            }
        }
        indexed
    }

    /// Invalidate the cache for a specific file (in-memory + disk).
    ///
    /// Used by the background analysis loop to clear stale cache entries
    /// when files change on disk.
    pub async fn invalidate_file(&self, path: &Path) {
        let mut cache = self.cache.write().await;
        cache.remove(path);
        self.disk_cache.invalidate(path).await;
    }

    /// Analyze a file asynchronously and cache the results
    ///
    /// Used by the background analysis loop to pre-warm the cache
    /// after file changes.
    pub async fn analyze_file_async(&self, path_str: &str) -> Option<()> {
        let path = self.resolve_path(path_str);
        self.check_file(&path).await?;
        Some(())
    }

    /// Get the current cache size
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Flush the disk cache manifest to disk. Call on shutdown.
    pub async fn flush_cache(&self) {
        self.disk_cache.flush_manifest().await;
    }
}
