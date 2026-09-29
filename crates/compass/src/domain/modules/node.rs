use std::collections::HashSet;
use std::path::PathBuf;

use crate::domain::modules::import::ModuleInfo;

/// A node in the module graph
#[derive(Debug, Clone)]
pub struct ModuleNode {
    /// Module name (e.g., "mypackage.submodule")
    pub name: String,
    /// File path if this is a file-based module
    pub path: Option<PathBuf>,
    /// Whether this is a package (__init__.py)
    pub is_package: bool,
    /// Modules this module imports
    pub imports: HashSet<String>,
    /// Modules that import this module
    pub imported_by: HashSet<String>,
    /// Type information for this module
    pub info: Option<ModuleInfo>,
}

impl ModuleNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            path: None,
            is_package: false,
            imports: HashSet::new(),
            imported_by: HashSet::new(),
            info: None,
        }
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.is_package = path
            .file_name()
            .map(|n| n == "__init__.py")
            .unwrap_or(false);
        self.path = Some(path);
        self
    }
}
