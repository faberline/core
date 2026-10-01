use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// A node in the import graph (represents a file)
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub path: PathBuf,
    pub imports: Vec<ImportEdge>,
}

/// An edge in the import graph (an import statement)
#[derive(Debug, Clone)]
pub struct ImportEdge {
    /// The raw import path as written in source
    pub import_path: String,
    /// Resolved absolute file path (if resolvable)
    pub resolved: Option<PathBuf>,
    /// Line number where the import appears
    pub line: u32,
}

/// Project-wide import dependency graph
pub struct ImportGraph {
    pub(crate) nodes: HashMap<PathBuf, GraphNode>,
    entry_points: Vec<PathBuf>,
}

impl Default for ImportGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            entry_points: Vec::new(),
        }
    }

    /// Remove a file from the graph
    pub fn remove_file(&mut self, path: &Path) {
        self.nodes.remove(path);
        self.entry_points.retain(|p| p != path);
    }

    /// Auto-detect entry points
    pub fn detect_entry_points(&mut self) {
        let names: HashSet<&str> = [
            "main.py",
            "app.py",
            "__main__.py",
            "index.ts",
            "index.tsx",
            "index.js",
            "index.jsx",
            "main.ts",
            "main.js",
            "main.rs",
            "lib.rs",
            "main.go",
        ]
        .into_iter()
        .collect();
        self.entry_points = self
            .nodes
            .keys()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| names.contains(n))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
    }

    /// Find all circular dependencies via DFS with recursion stack
    pub fn find_circular_dependencies(&self) -> Vec<Vec<PathBuf>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut on_stack = HashSet::new();
        for start in self.nodes.keys() {
            if !visited.contains(start) {
                self.dfs_cycles(start, &mut visited, &mut stack, &mut on_stack, &mut cycles);
            }
        }
        dedup_cycles(&mut cycles);
        cycles
    }

    fn dfs_cycles(
        &self,
        node: &Path,
        visited: &mut HashSet<PathBuf>,
        stack: &mut Vec<PathBuf>,
        on_stack: &mut HashSet<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        visited.insert(node.to_path_buf());
        stack.push(node.to_path_buf());
        on_stack.insert(node.to_path_buf());
        if let Some(gn) = self.nodes.get(node) {
            for edge in &gn.imports {
                if let Some(ref target) = edge.resolved {
                    if on_stack.contains(target) {
                        let pos = stack.iter().position(|p| p == target).unwrap();
                        cycles.push(stack[pos..].to_vec());
                    } else if !visited.contains(target) {
                        self.dfs_cycles(target, visited, stack, on_stack, cycles);
                    }
                }
            }
        }
        stack.pop();
        on_stack.remove(node);
    }

    /// Find files unreachable from any entry point
    pub fn find_unused_files(&self) -> Vec<PathBuf> {
        if self.entry_points.is_empty() {
            return Vec::new();
        }
        let reachable = self.bfs_reachable(&self.entry_points);
        let mut unused: Vec<PathBuf> = self
            .nodes
            .keys()
            .filter(|p| !reachable.contains(*p))
            .cloned()
            .collect();
        unused.sort();
        unused
    }

    fn bfs_reachable(&self, starts: &[PathBuf]) -> HashSet<PathBuf> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        for s in starts {
            if self.nodes.contains_key(s) {
                visited.insert(s.clone());
                queue.push_back(s.clone());
            }
        }
        while let Some(cur) = queue.pop_front() {
            if let Some(n) = self.nodes.get(&cur) {
                for e in &n.imports {
                    if let Some(ref t) = e.resolved {
                        if visited.insert(t.clone()) {
                            queue.push_back(t.clone());
                        }
                    }
                }
            }
        }
        visited
    }

    /// Get all direct dependencies of a file
    pub fn dependencies(&self, path: &Path) -> Vec<&ImportEdge> {
        self.nodes
            .get(path)
            .map(|n| n.imports.iter().collect())
            .unwrap_or_default()
    }

    /// Get all files that import this file (reverse dependencies)
    pub fn dependents(&self, path: &Path) -> Vec<PathBuf> {
        self.nodes
            .iter()
            .filter(|(_, n)| {
                n.imports
                    .iter()
                    .any(|e| e.resolved.as_deref() == Some(path))
            })
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn file_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.nodes
            .values()
            .flat_map(|n| &n.imports)
            .filter(|e| e.resolved.is_some())
            .count()
    }

    pub fn entry_points(&self) -> &[PathBuf] {
        &self.entry_points
    }

    pub fn set_entry_points(&mut self, entries: Vec<PathBuf>) {
        self.entry_points = entries;
    }
}

// -- Helpers -----------------------------------------------------------------

fn dedup_cycles(cycles: &mut Vec<Vec<PathBuf>>) {
    let mut seen: HashSet<Vec<PathBuf>> = HashSet::new();
    cycles.retain(|c| {
        if c.is_empty() {
            return false;
        }
        let min_idx = c
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.cmp(b))
            .unwrap()
            .0;
        let mut norm = Vec::with_capacity(c.len());
        norm.extend_from_slice(&c[min_idx..]);
        norm.extend_from_slice(&c[..min_idx]);
        seen.insert(norm)
    });
}
