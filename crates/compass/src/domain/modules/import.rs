use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::type_inference::Type;

/// Represents an import statement
#[derive(Debug, Clone)]
pub enum Import {
    /// import module
    Module {
        module: String,
        alias: Option<String>,
    },
    /// from module import name
    FromModule {
        module: String,
        names: Vec<ImportedName>,
    },
    /// from module import *
    WildcardImport { module: String },
}

/// A single imported name with optional alias
#[derive(Debug, Clone)]
pub struct ImportedName {
    pub name: String,
    pub alias: Option<String>,
}

impl Import {
    /// The names this import brings into scope, typed from the exports of
    /// `modules` (module path → info). `import m` binds a module instance;
    /// `from m import …` and `from m import *` bind the exported types
    /// (wildcards skip `_`-prefixed names); unknown modules bind nothing.
    pub(crate) fn resolve_in(
        &self,
        modules: &HashMap<String, ModuleInfo>,
    ) -> HashMap<String, Type> {
        let mut result = HashMap::new();

        match self {
            Import::Module { module, alias } => {
                // For `import foo`, we don't directly import types
                // The module name becomes available for attribute access
                let name = alias.as_ref().unwrap_or(module);
                result.insert(
                    name.clone(),
                    Type::Instance {
                        name: format!("module:{}", module),
                        module: Some(module.clone()),
                        type_args: vec![],
                    },
                );
            }
            Import::FromModule { module, names } => {
                if let Some(module_info) = modules.get(module) {
                    for imported_name in names {
                        let local_name =
                            imported_name.alias.as_ref().unwrap_or(&imported_name.name);

                        if let Some(ty) = module_info.exports.get(&imported_name.name) {
                            result.insert(local_name.clone(), ty.clone());
                        }
                    }
                }
            }
            Import::WildcardImport { module } => {
                if let Some(module_info) = modules.get(module) {
                    for (name, ty) in &module_info.exports {
                        // Skip private names
                        if !name.starts_with('_') {
                            result.insert(name.clone(), ty.clone());
                        }
                    }
                }
            }
        }

        result
    }
}

/// Loading state for a module (for circular import detection)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModuleLoadState {
    /// Module not yet loaded
    #[default]
    NotLoaded,
    /// Module is currently being loaded (used for circular import detection)
    Loading,
    /// Module fully loaded
    Loaded,
    /// Module failed to load
    Failed,
}

/// Module information including exported types
#[derive(Debug, Clone, Default)]
pub struct ModuleInfo {
    /// Module path (e.g., "collections.abc")
    pub path: String,
    /// File path if available
    pub file_path: Option<PathBuf>,
    /// Whether this was loaded from a stub file (.pyi)
    pub is_stub: bool,
    /// Exported types (name -> type)
    pub exports: HashMap<String, Type>,
    /// Re-exports from other modules
    pub reexports: HashMap<String, String>, // name -> source module
    /// Is this a package (__init__.py)?
    pub is_package: bool,
    /// Load state for circular import handling
    #[allow(dead_code)]
    load_state: ModuleLoadState,
    /// Submodules (for packages)
    pub submodules: Vec<String>,
}

impl ModuleInfo {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            file_path: None,
            is_stub: false,
            exports: HashMap::new(),
            reexports: HashMap::new(),
            is_package: false,
            load_state: ModuleLoadState::NotLoaded,
            submodules: Vec::new(),
        }
    }

    /// Create a module info from a file path
    pub fn from_file(path: &str, file_path: PathBuf) -> Self {
        let is_stub = file_path.extension().map_or(false, |ext| ext == "pyi");
        let is_package = file_path.file_name().map_or(false, |name| {
            name == "__init__.py" || name == "__init__.pyi"
        });

        Self {
            path: path.to_string(),
            file_path: Some(file_path),
            is_stub,
            exports: HashMap::new(),
            reexports: HashMap::new(),
            is_package,
            load_state: ModuleLoadState::NotLoaded,
            submodules: Vec::new(),
        }
    }

    /// Get an exported type by name
    pub fn get_export(&self, name: &str) -> Option<&Type> {
        self.exports.get(name)
    }

    /// Check if the module has been loaded
    pub fn is_loaded(&self) -> bool {
        self.load_state == ModuleLoadState::Loaded
    }
}

/// Indexed module entry for quick lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleIndexEntry {
    /// Full module path (e.g., "django.db.models")
    pub module_path: String,
    /// File path to the module source
    pub file_path: PathBuf,
    /// Whether this is a stub file
    pub is_stub: bool,
    /// Whether this is a package
    pub is_package: bool,
}

/// Parse an import statement from a tree-sitter node
pub fn parse_import(source: &str, node: &tree_sitter::Node) -> Option<Import> {
    let node_text = |n: &tree_sitter::Node| -> String {
        n.utf8_text(source.as_bytes()).unwrap_or("").to_string()
    };

    match node.kind() {
        "import_statement" => {
            // import foo, bar as baz
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                    let (module, alias) = if child.kind() == "aliased_import" {
                        let name = child.child_by_field_name("name").map(|n| node_text(&n));
                        let alias = child.child_by_field_name("alias").map(|n| node_text(&n));
                        (name.unwrap_or_default(), alias)
                    } else {
                        (node_text(&child), None)
                    };

                    return Some(Import::Module { module, alias });
                }
            }
            None
        }
        "import_from_statement" => {
            // from foo import bar, baz as qux
            let module = node
                .child_by_field_name("module_name")
                .map(|n| node_text(&n))
                .unwrap_or_default();

            // Check for wildcard import
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "wildcard_import" {
                    return Some(Import::WildcardImport { module });
                }
            }

            // Parse named imports
            let mut names = Vec::new();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "dotted_name" | "identifier" => {
                        names.push(ImportedName {
                            name: node_text(&child),
                            alias: None,
                        });
                    }
                    "aliased_import" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| node_text(&n))
                            .unwrap_or_default();
                        let alias = child.child_by_field_name("alias").map(|n| node_text(&n));
                        names.push(ImportedName { name, alias });
                    }
                    _ => {}
                }
            }

            if !names.is_empty() {
                Some(Import::FromModule { module, names })
            } else {
                None
            }
        }
        _ => None,
    }
}
