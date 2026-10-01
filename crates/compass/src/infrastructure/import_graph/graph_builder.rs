use crate::domain::import_graph::extract::extract_imports;
use crate::domain::import_graph::graph::{GraphNode, ImportEdge, ImportGraph};
use crate::infrastructure::import_graph::resolve::resolve_import;
use std::path::{Path, PathBuf};

impl ImportGraph {
    /// Build the graph from a list of files and their source contents
    pub fn build(files: &[(PathBuf, String)], project_root: &Path) -> Self {
        let mut g = Self::new();
        for (path, source) in files {
            g.add_file(path.clone(), source, project_root);
        }
        g.detect_entry_points();
        g
    }

    /// Add a single file to the graph (for incremental updates)
    pub fn add_file(&mut self, path: PathBuf, source: &str, project_root: &Path) {
        let extracted = extract_imports(source, &path);
        let edges = extracted
            .iter()
            .map(|ext| ImportEdge {
                import_path: ext.path.clone(),
                resolved: resolve_import(&ext.path, &path, project_root, ext.language),
                line: ext.line,
            })
            .collect();
        self.nodes.insert(
            path.clone(),
            GraphNode {
                path,
                imports: edges,
            },
        );
    }
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests;
