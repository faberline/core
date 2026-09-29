use std::collections::{HashMap, HashSet};

use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::syntax::ParsedFile;

impl PythonChecker {
    /// Check for unused imports
    pub(super) fn check_unused_imports(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut imports: HashMap<String, Range> = HashMap::new();
        let mut used_names: HashSet<String> = HashSet::new();

        // First pass: collect imports and used names
        file.walk(|node, _depth| {
            match node.kind() {
                "import_statement" => {
                    // import foo, bar
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "dotted_name" {
                            let name = file.node_text(&child);
                            // For "import foo.bar", we only track "foo"
                            let base_name = name.split('.').next().unwrap_or(name);
                            imports.insert(base_name.to_string(), Range::from_node(&child));
                        }
                    }
                }
                "import_from_statement" => {
                    // from foo import bar, baz
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                            let name = if child.kind() == "aliased_import" {
                                // from foo import bar as baz -> track "baz"
                                child
                                    .child_by_field_name("alias")
                                    .map(|n| file.node_text(&n))
                                    .unwrap_or_else(|| file.node_text(&child))
                            } else {
                                file.node_text(&child)
                            };
                            imports.insert(name.to_string(), Range::from_node(&child));
                        }
                    }
                }
                "identifier" => {
                    let name = file.node_text(node);
                    used_names.insert(name.to_string());
                }
                _ => {}
            }
            true
        });

        // Find unused imports
        for (name, range) in imports {
            if !used_names.contains(&name) {
                diagnostics.push(Diagnostic::warning(
                    range,
                    "PY102",
                    DiagnosticCategory::Names,
                    format!("Unused import: '{}'", name),
                ));
            }
        }

        diagnostics
    }

    /// Check for mutable default arguments
    pub(super) fn check_mutable_default(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "default_parameter" {
                if let Some(value) = node.child_by_field_name("value") {
                    let value_kind = value.kind();
                    if value_kind == "list" || value_kind == "dictionary" || value_kind == "set" {
                        diagnostics.push(Diagnostic::warning(
                            Range::from_node(&value),
                            "PY201",
                            DiagnosticCategory::Logic,
                            format!(
                                "Mutable default argument: {} literals are mutable and shared between calls",
                                value_kind
                            ),
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// Check for bare except clauses
    pub(super) fn check_bare_except(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "except_clause" {
                // Check if there's no exception type specified
                let has_type = node.children(&mut node.walk())
                    .any(|c| c.kind() == "identifier" || c.kind() == "tuple");

                if !has_type {
                    diagnostics.push(Diagnostic::warning(
                        Range::from_node(node),
                        "PY202",
                        DiagnosticCategory::Logic,
                        "Bare except clause catches all exceptions including KeyboardInterrupt and SystemExit",
                    ));
                }
            }
            true
        });

        diagnostics
    }

    /// Check for unreachable code after return/raise/break/continue
    pub(super) fn check_unreachable_code(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "block" {
                let mut found_terminal = false;
                let mut cursor = node.walk();

                for child in node.children(&mut cursor) {
                    if found_terminal && !child.kind().contains("comment") {
                        diagnostics.push(Diagnostic::warning(
                            Range::from_node(&child),
                            "PY203",
                            DiagnosticCategory::Logic,
                            "Unreachable code after return/raise/break/continue",
                        ));
                        break;
                    }

                    if matches!(
                        child.kind(),
                        "return_statement"
                            | "raise_statement"
                            | "break_statement"
                            | "continue_statement"
                    ) {
                        found_terminal = true;
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// Check for shadowed builtins
    pub(super) fn check_shadowed_builtins(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            // Check function/class definitions and assignments
            let name_node = match node.kind() {
                "function_definition" | "class_definition" => node.child_by_field_name("name"),
                "assignment" => {
                    // Get the left side of assignment
                    node.child_by_field_name("left")
                }
                _ => None,
            };

            if let Some(name_node) = name_node {
                if name_node.kind() == "identifier" {
                    let name = file.node_text(&name_node);
                    if self.builtins.contains(name) {
                        diagnostics.push(Diagnostic::warning(
                            Range::from_node(&name_node),
                            "PY104",
                            DiagnosticCategory::Names,
                            format!("Shadowing builtin name: '{}'", name),
                        ));
                    }
                }
            }

            true
        });

        diagnostics
    }
}
