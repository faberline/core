use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::modules::import::{Import, ModuleIndexEntry, ModuleInfo};
use crate::type_inference::Type;

/// Import resolver that manages module loading and type resolution
#[derive(Debug, Default)]
pub struct ImportResolver {
    /// Loaded modules (module path -> info)
    modules: HashMap<String, ModuleInfo>,
    /// Module index (module path -> file path) for quick lookups
    module_index: HashMap<String, ModuleIndexEntry>,
    /// Search paths for modules (in priority order)
    search_paths: Vec<PathBuf>,
    /// Current working directory
    cwd: PathBuf,
    /// Set of modules currently being loaded (for circular import detection)
    loading: HashSet<String>,
    /// Whether the index has been built
    indexed: bool,
}

impl ImportResolver {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            module_index: HashMap::new(),
            search_paths: vec![],
            cwd: PathBuf::new(),
            loading: HashSet::new(),
            indexed: false,
        }
    }

    /// Create a new resolver with the given search paths
    pub fn with_search_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            modules: HashMap::new(),
            module_index: HashMap::new(),
            search_paths: paths,
            cwd: PathBuf::new(),
            loading: HashSet::new(),
            indexed: false,
        }
    }

    /// Set search paths for module resolution
    pub fn set_search_paths(&mut self, paths: Vec<PathBuf>) {
        self.search_paths = paths;
        self.indexed = false; // Invalidate index
    }

    /// Add a search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
            self.indexed = false;
        }
    }

    /// Get the current search paths
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Set current working directory
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    /// Register a module's exports
    pub fn register_module(&mut self, module_path: &str, info: ModuleInfo) {
        self.modules.insert(module_path.to_string(), info);
    }

    /// Get a registered module
    pub fn get_module(&self, module_path: &str) -> Option<&ModuleInfo> {
        self.modules.get(module_path)
    }

    /// Get a mutable reference to a registered module
    pub fn get_module_mut(&mut self, module_path: &str) -> Option<&mut ModuleInfo> {
        self.modules.get_mut(module_path)
    }

    /// Check if a module is registered
    pub fn has_module(&self, module_path: &str) -> bool {
        self.modules.contains_key(module_path)
    }

    /// Get all registered module names
    pub fn module_names(&self) -> impl Iterator<Item = &String> {
        self.modules.keys()
    }

    /// Build the module index by scanning search paths
    pub fn build_index(&mut self) {
        self.module_index.clear();

        for search_path in &self.search_paths.clone() {
            self.index_directory(search_path, "");
        }

        // Also index cwd if not in search paths
        if !self.search_paths.contains(&self.cwd) && self.cwd.exists() {
            self.index_directory(&self.cwd.clone(), "");
        }

        self.indexed = true;
    }

    /// Index a directory recursively
    fn index_directory(&mut self, dir: &Path, prefix: &str) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden directories and common exclusions
            if name_str.starts_with('.') || name_str == "__pycache__" || name_str == "node_modules"
            {
                continue;
            }

            if path.is_dir() {
                // Check if it's a package
                let init_py = path.join("__init__.py");
                let init_pyi = path.join("__init__.pyi");

                let module_path = if prefix.is_empty() {
                    name_str.to_string()
                } else {
                    format!("{}.{}", prefix, name_str)
                };

                // Prefer .pyi over .py
                if init_pyi.exists() {
                    self.module_index.insert(
                        module_path.clone(),
                        ModuleIndexEntry {
                            module_path: module_path.clone(),
                            file_path: init_pyi,
                            is_stub: true,
                            is_package: true,
                        },
                    );
                    // Recurse into package
                    self.index_directory(&path, &module_path);
                } else if init_py.exists() {
                    self.module_index.insert(
                        module_path.clone(),
                        ModuleIndexEntry {
                            module_path: module_path.clone(),
                            file_path: init_py,
                            is_stub: false,
                            is_package: true,
                        },
                    );
                    // Recurse into package
                    self.index_directory(&path, &module_path);
                }
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                // Skip __init__ files (handled as packages)
                if stem == "__init__" {
                    continue;
                }

                let module_path = if prefix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}.{}", prefix, stem)
                };

                match ext {
                    Some("pyi") => {
                        // .pyi takes precedence
                        self.module_index.insert(
                            module_path.clone(),
                            ModuleIndexEntry {
                                module_path,
                                file_path: path,
                                is_stub: true,
                                is_package: false,
                            },
                        );
                    }
                    Some("py") => {
                        // Only add .py if no .pyi exists
                        if !self.module_index.contains_key(&module_path) {
                            self.module_index.insert(
                                module_path.clone(),
                                ModuleIndexEntry {
                                    module_path,
                                    file_path: path,
                                    is_stub: false,
                                    is_package: false,
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// List all indexed modules, optionally filtered by prefix
    pub fn list_modules(&self, prefix: Option<&str>) -> Vec<&ModuleIndexEntry> {
        match prefix {
            Some(p) => self
                .module_index
                .values()
                .filter(|e| e.module_path.starts_with(p))
                .collect(),
            None => self.module_index.values().collect(),
        }
    }

    /// Get module index entry
    pub fn get_index_entry(&self, module_path: &str) -> Option<&ModuleIndexEntry> {
        self.module_index.get(module_path)
    }

    /// Check if the index is built
    pub fn is_indexed(&self) -> bool {
        self.indexed
    }

    /// Check if a module is currently being loaded (for circular import detection)
    pub fn is_loading(&self, module_path: &str) -> bool {
        self.loading.contains(module_path)
    }

    /// Mark a module as being loaded
    pub fn start_loading(&mut self, module_path: &str) {
        self.loading.insert(module_path.to_string());
    }

    /// Mark a module as done loading
    pub fn finish_loading(&mut self, module_path: &str) {
        self.loading.remove(module_path);
    }

    /// Resolve an import and return the types it brings into scope
    pub fn resolve_import(&self, import: &Import) -> HashMap<String, Type> {
        import.resolve_in(&self.modules)
    }

    /// Resolve a module path to a file path
    /// Prefers .pyi (stub) files over .py files
    pub fn resolve_module_path(&self, module_path: &str) -> Option<PathBuf> {
        // First check the index
        if let Some(entry) = self.module_index.get(module_path) {
            return Some(entry.file_path.clone());
        }

        // Fall back to filesystem search
        self.resolve_module_path_filesystem(module_path)
    }

    /// Resolve a module path by searching the filesystem directly
    fn resolve_module_path_filesystem(&self, module_path: &str) -> Option<PathBuf> {
        let parts: Vec<&str> = module_path.split('.').collect();
        let relative_path = parts.join("/");

        // Check search paths
        for search_path in &self.search_paths {
            if let Some(path) = self.find_module_in_dir(search_path, &relative_path) {
                return Some(path);
            }
        }

        // Check relative to cwd
        self.find_module_in_dir(&self.cwd, &relative_path)
    }

    /// Find a module file in a directory, preferring .pyi over .py
    fn find_module_in_dir(&self, dir: &Path, relative_path: &str) -> Option<PathBuf> {
        // Try as a package with stub
        let package_stub = dir.join(relative_path).join("__init__.pyi");
        if package_stub.exists() {
            return Some(package_stub);
        }

        // Try as a package with .py
        let package_init = dir.join(relative_path).join("__init__.py");
        if package_init.exists() {
            return Some(package_init);
        }

        // Try as a stub module (.pyi) - prefer over .py
        let stub_file = dir.join(format!("{}.pyi", relative_path));
        if stub_file.exists() {
            return Some(stub_file);
        }

        // Try as a module (.py)
        let module_file = dir.join(format!("{}.py", relative_path));
        if module_file.exists() {
            return Some(module_file);
        }

        None
    }

    /// Get a resolved module, loading it if necessary
    /// Returns None if the module cannot be found or is currently being loaded (circular import)
    pub fn get_or_resolve_module(&mut self, module_path: &str) -> Option<&ModuleInfo> {
        // Check if already loaded
        if self.modules.contains_key(module_path) {
            return self.modules.get(module_path);
        }

        // Check for circular import
        if self.loading.contains(module_path) {
            // Return a placeholder for circular imports
            return None;
        }

        // Try to resolve the module path
        let file_path = self.resolve_module_path(module_path)?;

        // Create and register a basic module info
        // (actual parsing would be done by the type inferencer)
        let info = ModuleInfo::from_file(module_path, file_path);
        self.modules.insert(module_path.to_string(), info);
        self.modules.get(module_path)
    }

    /// Clear the resolver state
    pub fn clear(&mut self) {
        self.modules.clear();
        self.module_index.clear();
        self.loading.clear();
        self.indexed = false;
    }
}

#[cfg(test)]
mod tests;
