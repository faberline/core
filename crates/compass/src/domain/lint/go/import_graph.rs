use std::collections::HashMap;

use crate::syntax::ParsedFile;

// ============================================================================
// GoImportGraph — module-based import graph analysis
// ============================================================================

/// A simple module-based import graph built from parsed Go source files.
///
/// Records which modules are imported by which files so that callers can
/// perform impact analysis (e.g. "what re-exports a changed interface?").
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct GoImportGraph {
    /// file path → list of import paths
    imports: HashMap<String, Vec<String>>,
    /// import path → list of files that import it
    reverse: HashMap<String, Vec<String>>,
}

#[allow(dead_code)]
impl GoImportGraph {
    /// Create a new, empty import graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add all import paths found in `file` to the graph.
    pub fn add_file(&mut self, file_path: &str, file: &ParsedFile) {
        let imports = collect_import_paths(file);
        for imp in &imports {
            self.reverse
                .entry(imp.clone())
                .or_default()
                .push(file_path.to_string());
        }
        self.imports.insert(file_path.to_string(), imports);
    }

    /// Return all files that import the given module path.
    pub fn files_importing(&self, module_path: &str) -> Vec<&str> {
        self.reverse
            .get(module_path)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Return all import paths used by the given file.
    pub fn imports_of(&self, file_path: &str) -> Vec<&str> {
        self.imports
            .get(file_path)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

/// Extract all string import paths from a parsed Go file.
#[allow(dead_code)]
fn collect_import_paths(file: &ParsedFile) -> Vec<String> {
    let mut paths = Vec::new();
    file.walk(|node, _depth| {
        if node.kind() == "import_spec" {
            // The path is the `interpreted_string_literal` child
            if let Some(path_node) = node.child_by_field_name("path") {
                let raw = file.node_text(&path_node);
                // Strip surrounding quotes
                let trimmed = raw.trim_matches('"');
                if !trimmed.is_empty() {
                    paths.push(trimmed.to_string());
                }
            }
        }
        true
    });
    paths
}
