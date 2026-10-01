use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

// ============================================================================
// DependencyGraph
// ============================================================================

/// Directed import graph: `dependency → {set of importers}`.
///
/// When file A imports file B, there is an edge `B → A` in this graph, meaning
/// "if B changes, A may be affected".
///
/// The graph is intentionally kept as a simple adjacency list rather than a
/// full module-resolution system, which keeps it fast and workspace-agnostic.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// dependency → importers
    pub(super) reverse_edges: HashMap<PathBuf, HashSet<PathBuf>>,
    /// importer → dependencies
    pub(super) forward_edges: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `importer` depends on `dependency`.
    ///
    /// This is the forward direction (`importer` imports `dependency`).
    /// The reverse edge is added automatically.
    pub fn add_edge(&mut self, dependency: PathBuf, importer: PathBuf) {
        self.reverse_edges
            .entry(dependency.clone())
            .or_default()
            .insert(importer.clone());
        self.forward_edges
            .entry(importer)
            .or_default()
            .insert(dependency);
    }

    /// Remove all edges for a deleted file (both as importer and dependency).
    pub fn remove_file(&mut self, path: &Path) {
        // Remove as dependency (clear all importer reverse-edges pointing here)
        self.reverse_edges.remove(path);
        // Remove as importer in reverse_edges
        for importers in self.reverse_edges.values_mut() {
            importers.remove(path);
        }
        // Remove forward edges
        self.forward_edges.remove(path);
    }

    /// Return the set of files that directly import `dependency`.
    pub fn direct_importers(&self, dependency: &Path) -> Vec<PathBuf> {
        self.reverse_edges
            .get(dependency)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Return the set of files that `importer` directly depends on.
    pub fn direct_dependencies(&self, importer: &Path) -> Vec<PathBuf> {
        self.forward_edges
            .get(importer)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Compute all files transitively affected by changes to `changed_files`.
    ///
    /// Uses BFS over the reverse-edge graph.  Returns the set of all files
    /// that need re-analysis, including the originally changed files.
    pub fn transitive_importers(&self, changed_files: &[PathBuf]) -> HashSet<PathBuf> {
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();

        for f in changed_files {
            if visited.insert(f.clone()) {
                queue.push_back(f.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if let Some(importers) = self.reverse_edges.get(&current) {
                for importer in importers {
                    if visited.insert(importer.clone()) {
                        queue.push_back(importer.clone());
                    }
                }
            }
        }

        visited
    }

    /// Total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.reverse_edges.values().map(|s| s.len()).sum()
    }
}
