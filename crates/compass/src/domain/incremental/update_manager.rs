use std::path::{Path, PathBuf};

use super::dependency_graph::DependencyGraph;
use super::dirty_file_tracker::DirtyFileTracker;
use super::file_change_kind::FileChangeKind;

// ============================================================================
// IncrementalUpdateManager
// ============================================================================

/// Orchestrates dirty-file tracking and dependency-aware invalidation.
///
/// The daemon calls `file_changed()` for each event from the watch bridge,
/// then calls `drain_dirty_files()` to obtain the minimal set of files to
/// re-analyze.  The dependency graph is updated separately by the indexer
/// after it parses import statements.
pub struct IncrementalUpdateManager {
    tracker: DirtyFileTracker,
    dep_graph: DependencyGraph,
}

impl IncrementalUpdateManager {
    /// Create a new manager with empty tracker and dependency graph.
    pub fn new() -> Self {
        Self {
            tracker: DirtyFileTracker::new(),
            dep_graph: DependencyGraph::new(),
        }
    }

    /// Record a file-system change event.
    pub fn file_changed(&mut self, path: PathBuf, kind: FileChangeKind) {
        if kind == FileChangeKind::Deleted {
            self.dep_graph.remove_file(&path);
        }
        self.tracker.mark_dirty(path, kind);
    }

    /// Register an import relationship: `importer` imports `dependency`.
    ///
    /// Call this after (re-)analyzing `importer` to keep the graph current.
    pub fn add_import_edge(&mut self, dependency: PathBuf, importer: PathBuf) {
        self.dep_graph.add_edge(dependency, importer);
    }

    /// Remove all import edges originating from `importer`.
    ///
    /// Call this before re-analyzing `importer` so that stale edges don't
    /// linger after imports are removed or renamed.
    pub fn clear_importer_edges(&mut self, importer: &Path) {
        let deps = self.dep_graph.direct_dependencies(importer);
        for dep in &deps {
            if let Some(set) = self.dep_graph.reverse_edges.get_mut(dep) {
                set.remove(importer);
            }
        }
        self.dep_graph.forward_edges.remove(importer);
    }

    /// Return `true` when there are files pending re-analysis.
    pub fn has_pending_work(&self) -> bool {
        self.tracker.has_dirty()
    }

    /// Atomically consume all dirty files, expand via dependency-aware
    /// invalidation, and return the full set that must be re-analyzed.
    ///
    /// The return value is sorted for deterministic output.
    pub fn drain_dirty_files(&mut self) -> Vec<PathBuf> {
        let dirty_map = self.tracker.drain();
        if dirty_map.is_empty() {
            return vec![];
        }

        let dirty_list: Vec<PathBuf> = dirty_map.into_keys().collect();

        // BFS: expand dirty set to include all transitive importers.
        let expanded = self.dep_graph.transitive_importers(&dirty_list);

        let mut result: Vec<PathBuf> = expanded.into_iter().collect();
        result.sort();
        result
    }

    /// Access the dependency graph (read-only).
    pub fn dep_graph(&self) -> &DependencyGraph {
        &self.dep_graph
    }

    /// Access the dirty file tracker (read-only).
    pub fn tracker(&self) -> &DirtyFileTracker {
        &self.tracker
    }
}

impl Default for IncrementalUpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
