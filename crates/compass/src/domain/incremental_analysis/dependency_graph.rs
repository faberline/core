use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// ============================================================================
// Dependency Graph
// ============================================================================

/// Graph of file dependencies for incremental analysis.
pub struct DependencyGraph {
    /// Direct dependencies: file -> files it depends on
    dependencies: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Reverse dependencies: file -> files that depend on it
    dependents: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl DependencyGraph {
    /// Create a new dependency graph.
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }

    /// Add a dependency.
    pub fn add_dependency(&mut self, from: PathBuf, to: PathBuf) {
        self.dependencies
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.dependents.entry(to).or_default().insert(from);
    }

    /// Remove a file and its dependencies.
    pub fn remove_file(&mut self, file: &PathBuf) {
        // Remove from dependencies
        if let Some(deps) = self.dependencies.remove(file) {
            for dep in deps {
                if let Some(dependents) = self.dependents.get_mut(&dep) {
                    dependents.remove(file);
                }
            }
        }

        // Remove from dependents
        if let Some(deps) = self.dependents.remove(file) {
            for dep in deps {
                if let Some(dependencies) = self.dependencies.get_mut(&dep) {
                    dependencies.remove(file);
                }
            }
        }
    }

    /// Get files affected by changes to a file.
    pub fn get_affected_files(&self, changed: &PathBuf) -> HashSet<PathBuf> {
        let mut affected = HashSet::new();
        let mut queue = vec![changed.clone()];

        while let Some(file) = queue.pop() {
            if affected.insert(file.clone()) {
                if let Some(dependents) = self.dependents.get(&file) {
                    queue.extend(dependents.iter().cloned());
                }
            }
        }

        affected
    }

    /// Get direct dependencies of a file.
    pub fn get_dependencies(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.dependencies.get(file)
    }

    /// Get direct dependents of a file.
    pub fn get_dependents(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.dependents.get(file)
    }

    /// Check if graph contains a file.
    pub fn contains(&self, file: &PathBuf) -> bool {
        self.dependencies.contains_key(file) || self.dependents.contains_key(file)
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
