use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Import graph for dependency tracking.
#[derive(Debug, Clone, Default)]
pub struct ImportGraph {
    /// Edges: file -> files it imports from
    edges: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Reverse edges: file -> files that import it
    reverse_edges: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl ImportGraph {
    /// Create a new import graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an import edge.
    pub fn add_import(&mut self, from: PathBuf, to: PathBuf) {
        self.edges
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.reverse_edges.entry(to).or_default().insert(from);
    }

    /// Get files imported by a file.
    pub fn imports(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.edges.get(file)
    }

    /// Get files that import a file.
    pub fn imported_by(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.reverse_edges.get(file)
    }

    /// Return all file paths known to this graph.
    pub fn all_files(&self) -> Vec<PathBuf> {
        let mut files: HashSet<PathBuf> = self.edges.keys().cloned().collect();
        for targets in self.edges.values() {
            files.extend(targets.iter().cloned());
        }
        files.into_iter().collect()
    }

    /// Topological sort for analysis order.
    pub fn topological_sort(&self) -> Vec<PathBuf> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        for file in self.edges.keys() {
            self.visit(file, &mut visited, &mut temp_visited, &mut result);
        }

        // DFS post-order gives us the correct topological order
        // (dependencies before dependents)
        result
    }

    fn visit(
        &self,
        file: &PathBuf,
        visited: &mut HashSet<PathBuf>,
        temp_visited: &mut HashSet<PathBuf>,
        result: &mut Vec<PathBuf>,
    ) {
        if visited.contains(file) {
            return;
        }
        if temp_visited.contains(file) {
            // Cycle detected - skip
            return;
        }

        temp_visited.insert(file.clone());

        if let Some(imports) = self.imports(file) {
            for imported in imports {
                self.visit(imported, visited, temp_visited, result);
            }
        }

        temp_visited.remove(file);
        visited.insert(file.clone());
        result.push(file.clone());
    }
}
