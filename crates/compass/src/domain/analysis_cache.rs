//! The analysis-cache port: per-file analysis kept across daemon runs.
//!
//! Entries are keyed by file path and content hash, so an entry stored for
//! other content never loads. The disk-backed implementation is `DiskCache`
//! in infrastructure/analysis_cache.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use crate::domain::diagnostic::model::Diagnostic;
use crate::domain::semantic_model::model::SemanticModel;

/// The boxed future an [`AnalysisCache`] method returns.
pub type CacheFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Persists the semantic model and diagnostics of analysed files.
///
/// Every method is best effort: a cache that cannot read or write behaves
/// as a miss and never fails the analysis.
pub trait AnalysisCache: Send + Sync {
    /// The semantic model and diagnostics stored for `path`, if they were
    /// stored for content with hash `content_hash`.
    fn load<'a>(
        &'a self,
        path: &'a Path,
        content_hash: u64,
    ) -> CacheFuture<'a, Option<(SemanticModel, Vec<Diagnostic>)>>;

    /// Store the analysis of `path` at content hash `content_hash`.
    fn store<'a>(
        &'a self,
        path: &'a Path,
        content_hash: u64,
        semantic_model: &'a SemanticModel,
        diagnostics: &'a [Diagnostic],
    ) -> CacheFuture<'a, ()>;

    /// Drop the entry for `path`.
    fn invalidate<'a>(&'a self, path: &'a Path) -> CacheFuture<'a, ()>;

    /// Write any buffered index state out, e.g. on shutdown.
    fn flush(&self) -> CacheFuture<'_, ()>;
}
