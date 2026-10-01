use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Parser;

use crate::domain::modules::import::ModuleInfo;
use crate::domain::stubs::bundled::bundled_stubs;
use crate::infrastructure::stubs::loader::StubLoader;

impl StubLoader {
    pub fn new() -> Self {
        Self {
            stubs: HashMap::new(),
            stub_paths: vec![],
            builtins_loaded: false,
        }
    }

    /// Set stub search paths
    pub fn set_stub_paths(&mut self, paths: Vec<PathBuf>) {
        self.stub_paths = paths;
    }

    /// Add a stub search path
    pub fn add_stub_path(&mut self, path: PathBuf) {
        if !self.stub_paths.contains(&path) {
            self.stub_paths.push(path);
        }
    }

    /// Load builtin stubs
    pub fn load_builtins(&mut self) {
        if self.builtins_loaded {
            return;
        }

        self.stubs.extend(bundled_stubs());

        self.builtins_loaded = true;
    }

    /// Get or load a stub for a module (loads from .pyi files if not cached)
    /// Returns None if no stub exists for the module
    /// Caller should check inline types first for proper priority:
    /// Priority: inline types > .pyi stubs > typeshed > inferred
    pub fn get_or_load_stub(&mut self, module_path: &str) -> Option<&ModuleInfo> {
        // Check already loaded stubs
        if self.stubs.contains_key(module_path) {
            return self.stubs.get(module_path);
        }

        // Try to load from stub paths (.pyi files)
        if let Some(stub_path) = self.find_stub_file(module_path) {
            if let Some(info) = self.parse_stub_file(&stub_path) {
                self.stubs.insert(module_path.to_string(), info);
                return self.stubs.get(module_path);
            }
        }

        // Return bundled typeshed stub if available
        self.stubs.get(module_path)
    }

    /// Check if a package is typed (has py.typed marker)
    #[allow(dead_code)]
    pub fn is_typed_package(&self, package_path: &Path) -> bool {
        package_path.join("py.typed").exists()
    }

    /// Get stub for a module
    pub fn get_stub(&self, module_path: &str) -> Option<&ModuleInfo> {
        self.stubs.get(module_path)
    }

    /// Check if a stub exists for a module
    pub fn has_stub(&self, module_path: &str) -> bool {
        self.stubs.contains_key(module_path)
    }

    /// Try to load a stub file for a module
    #[allow(dead_code)]
    pub fn load_stub(&mut self, module_path: &str) -> Option<&ModuleInfo> {
        if self.stubs.contains_key(module_path) {
            return self.stubs.get(module_path);
        }

        // Try to find and load the stub
        if let Some(stub_path) = self.find_stub_file(module_path) {
            if let Some(info) = self.parse_stub_file(&stub_path) {
                self.stubs.insert(module_path.to_string(), info);
                return self.stubs.get(module_path);
            }
        }

        None
    }

    /// Find stub file for a module
    fn find_stub_file(&self, module_path: &str) -> Option<PathBuf> {
        let parts: Vec<&str> = module_path.split('.').collect();
        let relative_path = parts.join("/");

        for stub_path in &self.stub_paths {
            // Try as package stub
            let package_stub = stub_path.join(&relative_path).join("__init__.pyi");
            if package_stub.exists() {
                return Some(package_stub);
            }

            // Try as module stub
            let module_stub = stub_path.join(format!("{}.pyi", relative_path));
            if module_stub.exists() {
                return Some(module_stub);
            }
        }

        None
    }

    /// Parse a stub file using tree-sitter
    fn parse_stub_file(&self, path: &Path) -> Option<ModuleInfo> {
        let source = fs::read_to_string(path).ok()?;
        let module_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .ok()?;

        let tree = parser.parse(&source, None)?;
        let root = tree.root_node();

        let mut info = ModuleInfo::new(module_name);
        self.parse_stub_definitions(&source, &root, &mut info);

        Some(info)
    }

    /// Get all loaded modules
    pub fn modules(&self) -> impl Iterator<Item = (&String, &ModuleInfo)> {
        self.stubs.iter()
    }
}
