use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::domain::modules::import::ModuleInfo;
use crate::domain::modules::node::ModuleNode;

/// Module dependency graph
#[derive(Debug, Default)]
pub struct ModuleGraph {
    /// All modules in the graph
    modules: HashMap<String, ModuleNode>,
    /// Root modules (entry points)
    roots: HashSet<String>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            roots: HashSet::new(),
        }
    }

    /// Add a module to the graph
    pub fn add_module(&mut self, name: &str, path: Option<PathBuf>) -> &mut ModuleNode {
        self.modules.entry(name.to_string()).or_insert_with(|| {
            let mut node = ModuleNode::new(name);
            if let Some(p) = path {
                node = node.with_path(p);
            }
            node
        })
    }

    /// Add an import relationship
    pub fn add_import(&mut self, from_module: &str, to_module: &str) {
        // Ensure both modules exist
        self.add_module(from_module, None);
        self.add_module(to_module, None);

        // Add the import relationship
        if let Some(from) = self.modules.get_mut(from_module) {
            from.imports.insert(to_module.to_string());
        }
        if let Some(to) = self.modules.get_mut(to_module) {
            to.imported_by.insert(from_module.to_string());
        }
    }

    /// Set a module as a root (entry point)
    pub fn set_root(&mut self, name: &str) {
        self.roots.insert(name.to_string());
    }

    /// Get a module by name
    pub fn get_module(&self, name: &str) -> Option<&ModuleNode> {
        self.modules.get(name)
    }

    /// Get a mutable module by name
    pub fn get_module_mut(&mut self, name: &str) -> Option<&mut ModuleNode> {
        self.modules.get_mut(name)
    }

    /// Check if a module exists
    pub fn has_module(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    /// Get all module names
    pub fn module_names(&self) -> impl Iterator<Item = &String> {
        self.modules.keys()
    }

    /// Get all modules
    pub fn modules(&self) -> impl Iterator<Item = (&String, &ModuleNode)> {
        self.modules.iter()
    }

    /// Detect circular imports
    /// Returns a list of cycles (each cycle is a list of module names)
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for name in self.modules.keys() {
            if !visited.contains(name) {
                self.dfs_cycles(name, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycles(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(name.to_string());
        rec_stack.insert(name.to_string());
        path.push(name.to_string());

        if let Some(node) = self.modules.get(name) {
            for import in &node.imports {
                if !visited.contains(import) {
                    self.dfs_cycles(import, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(import) {
                    // Found a cycle - extract the cycle from path
                    if let Some(pos) = path.iter().position(|n| n == import) {
                        let cycle: Vec<String> = path[pos..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(name);
    }

    /// Topological sort of modules for analysis order
    /// Returns modules in order such that dependencies come before dependents
    /// Returns None if there are circular dependencies
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result = Vec::new();

        // Initialize in-degrees
        for name in self.modules.keys() {
            in_degree.insert(name.clone(), 0);
        }

        // Calculate in-degrees (number of imports each module has)
        for (name, node) in &self.modules {
            for import in &node.imports {
                if self.modules.contains_key(import) {
                    *in_degree.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }

        // Find modules with no imports (in-degree = 0)
        for (name, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(name.clone());
            }
        }

        // Process modules
        while let Some(name) = queue.pop_front() {
            result.push(name.clone());

            if let Some(node) = self.modules.get(&name) {
                // Decrease in-degree for modules that import this one
                for importer in &node.imported_by {
                    if let Some(degree) = in_degree.get_mut(importer) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(importer.clone());
                        }
                    }
                }
            }
        }

        // If we processed all modules, return the order; otherwise there's a cycle
        if result.len() == self.modules.len() {
            Some(result)
        } else {
            None
        }
    }

    /// Get modules that need to be analyzed before the given module
    pub fn get_dependencies(&self, name: &str) -> HashSet<String> {
        let mut deps = HashSet::new();
        if let Some(node) = self.modules.get(name) {
            for import in &node.imports {
                deps.insert(import.clone());
            }
        }
        deps
    }

    /// Get modules that depend on the given module
    pub fn get_dependents(&self, name: &str) -> HashSet<String> {
        let mut dependents = HashSet::new();
        if let Some(node) = self.modules.get(name) {
            for importer in &node.imported_by {
                dependents.insert(importer.clone());
            }
        }
        dependents
    }

    /// Get all transitive dependencies of a module
    pub fn get_transitive_dependencies(&self, name: &str) -> HashSet<String> {
        let mut deps = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        if let Some(node) = self.modules.get(name) {
            for import in &node.imports {
                queue.push_back(import.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if deps.insert(current.clone()) {
                if let Some(node) = self.modules.get(&current) {
                    for import in &node.imports {
                        if !deps.contains(import) {
                            queue.push_back(import.clone());
                        }
                    }
                }
            }
        }

        deps
    }

    /// Get all modules that transitively depend on the given module
    ///
    /// This is the reverse of `get_transitive_dependencies` and is used
    /// for incremental invalidation - when a module changes, all modules
    /// that (transitively) import it need to be re-analyzed.
    pub fn get_transitive_dependents(&self, name: &str) -> HashSet<String> {
        let mut dependents = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        if let Some(node) = self.modules.get(name) {
            for importer in &node.imported_by {
                queue.push_back(importer.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if dependents.insert(current.clone()) {
                if let Some(node) = self.modules.get(&current) {
                    for importer in &node.imported_by {
                        if !dependents.contains(importer) {
                            queue.push_back(importer.clone());
                        }
                    }
                }
            }
        }

        dependents
    }

    /// Get all modules affected by a change to the given module
    ///
    /// Returns the changed module plus all its transitive dependents,
    /// sorted in reverse topological order (dependents before dependencies)
    /// for proper re-analysis ordering.
    pub fn get_affected_modules(&self, changed: &str) -> Vec<String> {
        let mut affected = self.get_transitive_dependents(changed);
        affected.insert(changed.to_string());

        // Sort in reverse topological order so dependents are analyzed after
        // the modules they depend on
        let mut sorted: Vec<String> = affected.into_iter().collect();
        sorted.sort_by(|a, b| {
            // If a imports b (a depends on b), b should come first
            // So if a's dependencies include b, a > b (a comes after b)
            let a_depends_on_b = self
                .get_module(a)
                .map(|n| n.imports.contains(b))
                .unwrap_or(false);
            let b_depends_on_a = self
                .get_module(b)
                .map(|n| n.imports.contains(a))
                .unwrap_or(false);

            if a_depends_on_b && !b_depends_on_a {
                std::cmp::Ordering::Greater
            } else if b_depends_on_a && !a_depends_on_b {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        sorted
    }

    /// Remove a module from the graph (e.g., when a file is deleted)
    pub fn remove_module(&mut self, name: &str) {
        // Remove from other modules' imports/imported_by
        if let Some(node) = self.modules.remove(name) {
            // Remove this module from the imports of modules it imports
            for import in &node.imports {
                if let Some(imported) = self.modules.get_mut(import) {
                    imported.imported_by.remove(name);
                }
            }
            // Remove this module from the imported_by of modules that import it
            for importer in &node.imported_by {
                if let Some(importing) = self.modules.get_mut(importer) {
                    importing.imports.remove(name);
                }
            }
        }

        self.roots.remove(name);
    }

    /// Update module info
    pub fn set_module_info(&mut self, name: &str, info: ModuleInfo) {
        if let Some(node) = self.modules.get_mut(name) {
            node.info = Some(info);
        }
    }

    /// Clear all imports for a module (before re-analyzing)
    pub fn clear_imports(&mut self, name: &str) {
        if let Some(node) = self.modules.get_mut(name) {
            // Remove from imported_by of modules this module imports
            let imports: Vec<String> = node.imports.drain().collect();
            for import in imports {
                if let Some(imported) = self.modules.get_mut(&import) {
                    imported.imported_by.remove(name);
                }
            }
        }
    }

    /// Find module by file path
    pub fn find_by_path(&self, path: &Path) -> Option<&String> {
        self.modules.iter().find_map(|(name, node)| {
            if node.path.as_deref() == Some(path) {
                Some(name)
            } else {
                None
            }
        })
    }

    /// Convert a file path to a module name
    pub fn path_to_module_name(path: &Path, root: &Path) -> Option<String> {
        let relative = path.strip_prefix(root).ok()?;
        let stem = relative.with_extension("");

        let parts: Vec<&str> = stem
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        if parts.is_empty() {
            return None;
        }

        // Handle __init__.py -> package name
        if parts.last() == Some(&"__init__") {
            Some(parts[..parts.len() - 1].join("."))
        } else {
            Some(parts.join("."))
        }
    }

    /// Convert a module name to possible file paths
    pub fn module_name_to_paths(name: &str, root: &Path) -> Vec<PathBuf> {
        let parts: Vec<&str> = name.split('.').collect();
        let relative_path = parts.join("/");

        vec![
            // Try as module file
            root.join(format!("{}.py", relative_path)),
            // Try as package __init__
            root.join(format!("{}/__init__.py", relative_path)),
            // Try as stub file
            root.join(format!("{}.pyi", relative_path)),
        ]
    }
}

#[cfg(test)]
mod tests;
