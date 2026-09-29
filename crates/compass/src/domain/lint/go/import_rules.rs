use super::GoChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Range};
use crate::syntax::ParsedFile;

impl GoChecker {
    /// GO002: Blank import — `import _ "pkg"` flagged as info
    pub(super) fn check_blank_import(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "import_spec" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if file.node_text(&name_node) == "_" {
                        diagnostics.push(Diagnostic::new(
                            Range::from_node(node),
                            DiagnosticSeverity::Information,
                            "GO002",
                            DiagnosticCategory::Style,
                            "Blank import '_ \"pkg\"' — ensure side-effect import is intentional",
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// GO010: Unused import (non-blank, non-side-effect import that is never
    /// referenced by any identifier in the file body)
    ///
    /// Tree-sitter allows us to collect all import names and verify each
    /// appears at least once as a selector prefix in the source text.
    pub(super) fn check_unused_import(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Collect (local_name, path, node) for each non-blank import spec.
        let mut imports: Vec<(String, String, tree_sitter::Range)> = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "import_spec" {
                // Skip blank imports (handled by GO002)
                if let Some(name_node) = node.child_by_field_name("name") {
                    if file.node_text(&name_node) == "_" {
                        return true;
                    }
                    // Skip dot imports
                    if file.node_text(&name_node) == "." {
                        return true;
                    }
                }

                let path_node = match node.child_by_field_name("path") {
                    Some(p) => p,
                    None => return true,
                };
                let raw_path = file.node_text(&path_node);
                let path = raw_path.trim_matches('"').to_string();

                // Derive the local name (last component of the path, or alias)
                let local_name = if let Some(name_node) = node.child_by_field_name("name") {
                    file.node_text(&name_node).to_string()
                } else {
                    path.split('/').last().unwrap_or(&path).to_string()
                };

                imports.push((local_name, path, node.range()));
            }
            true
        });

        // For each import, check if its local name appears as a selector prefix
        // anywhere in the source (simple text scan).
        for (local_name, _path, range) in &imports {
            let selector = format!("{}.", local_name);
            // Also check for `local_name` used directly as a type (e.g. `http.Handler`)
            if !file.source.contains(&selector) {
                let ts_range = crate::diagnostic::Range::new(
                    crate::diagnostic::Position::new(
                        range.start_point.row as u32,
                        range.start_point.column as u32,
                    ),
                    crate::diagnostic::Position::new(
                        range.end_point.row as u32,
                        range.end_point.column as u32,
                    ),
                );
                diagnostics.push(Diagnostic::new(
                    ts_range,
                    DiagnosticSeverity::Warning,
                    "GO010",
                    DiagnosticCategory::Names,
                    format!("Imported package '{}' appears to be unused", local_name),
                ));
            }
        }

        diagnostics
    }
}
