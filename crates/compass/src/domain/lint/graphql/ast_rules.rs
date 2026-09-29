use std::collections::{HashMap, HashSet};

use super::{lr, GraphqlChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::syntax::ParsedFile;

impl GraphqlChecker {
    // =========================================================================
    // AST-based checks (tree-sitter-graphql node kinds)
    // =========================================================================

    /// GQ001 via AST: report tree-sitter syntax errors directly.
    pub(super) fn ast_check_syntax(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for err in file.collect_errors() {
            let (row, col) = err.start_position;
            let pos = Position::new((row - 1) as u32, (col - 1) as u32);
            diags.push(Diagnostic::new(
                Range::new(pos, pos),
                DiagnosticSeverity::Error,
                "GQ001",
                DiagnosticCategory::Syntax,
                "GraphQL syntax error",
            ));
        }
        diags
    }

    /// GQ002 via AST: collect defined type names and flag undefined references.
    pub(super) fn ast_check_undefined_types(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        // Built-in scalar types
        let mut defined: HashSet<String> = ["String", "Int", "Float", "Boolean", "ID"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut refs: Vec<(String, u32)> = Vec::new();

        let type_def_kinds: &[&str] = &[
            "object_type_definition",
            "interface_type_definition",
            "union_type_definition",
            "enum_type_definition",
            "input_object_type_definition",
            "scalar_type_definition",
        ];

        file.walk(|node, _depth| {
            let kind = node.kind();

            // Collect type definitions
            if type_def_kinds.contains(&kind) {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        defined.insert(name.to_string());
                    }
                }
            }

            // Collect named type references (fields, arguments, etc.)
            if kind == "named_type" {
                if let Ok(name) = node.utf8_text(source) {
                    let line = node.start_position().row as u32;
                    refs.push((name.to_string(), line));
                }
            }

            true
        });

        for (tn, line) in &refs {
            if !defined.contains(tn.as_str()) {
                diags.push(Diagnostic::new(
                    lr(*line),
                    DiagnosticSeverity::Warning,
                    "GQ002",
                    DiagnosticCategory::Names,
                    format!("Reference to undefined type '{}'", tn),
                ));
            }
        }
        diags
    }

    /// GQ003 via AST: find @deprecated directives on field definitions.
    pub(super) fn ast_check_deprecated(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        file.walk(|node, _depth| {
            if node.kind() == "directive" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if name_node.utf8_text(source).ok() == Some("deprecated") {
                        let line = node.start_position().row as u32;
                        diags.push(Diagnostic::new(
                            lr(line),
                            DiagnosticSeverity::Warning,
                            "GQ003",
                            DiagnosticCategory::Logic,
                            "Field marked @deprecated — consider removing or updating usage",
                        ));
                    }
                }
            }
            true
        });
        diags
    }

    /// GQ004 via AST: measure selection set nesting depth.
    pub(super) fn ast_check_deep_nesting(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut warned: HashSet<u32> = HashSet::new();

        file.walk(|node, depth| {
            if node.kind() == "selection_set" && depth > 5 {
                let line = node.start_position().row as u32;
                if warned.insert(line) {
                    diags.push(Diagnostic::new(
                        lr(line),
                        DiagnosticSeverity::Warning,
                        "GQ004",
                        DiagnosticCategory::Style,
                        format!("Nesting depth {} exceeds maximum of 5", depth),
                    ));
                }
            }
            true
        });
        diags
    }

    /// GQ005 via AST: type/interface/input definitions without a description.
    pub(super) fn ast_check_missing_descriptions(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        let def_kinds: &[&str] = &[
            "object_type_definition",
            "interface_type_definition",
            "input_object_type_definition",
        ];

        file.walk(|node, _depth| {
            if def_kinds.contains(&node.kind()) {
                // Check for a description string_value child
                let has_desc = (0..node.child_count())
                    .filter_map(|i| node.child(i))
                    .any(|c| c.kind() == "string_value" || c.kind() == "block_string_value");
                if !has_desc {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = name_node.utf8_text(source).unwrap_or("");
                        if !matches!(name, "Query" | "Mutation" | "Subscription") {
                            let line = node.start_position().row as u32;
                            diags.push(Diagnostic::new(
                                lr(line),
                                DiagnosticSeverity::Hint,
                                "GQ005",
                                DiagnosticCategory::Style,
                                format!("Type '{}' has no description", name),
                            ));
                        }
                    }
                }
            }
            true
        });
        diags
    }

    /// GQ006 via AST: fragment defined but never spread.
    pub(super) fn ast_check_unused_fragments(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();
        let mut defs: HashMap<String, u32> = HashMap::new();
        let mut spreads: HashSet<String> = HashSet::new();

        file.walk(|node, _depth| {
            match node.kind() {
                "fragment_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            defs.insert(name.to_string(), node.start_position().row as u32);
                        }
                    }
                }
                "fragment_spread" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            spreads.insert(name.to_string());
                        }
                    }
                }
                _ => {}
            }
            true
        });

        for (name, line) in &defs {
            if !spreads.contains(name.as_str()) {
                diags.push(Diagnostic::new(
                    lr(*line),
                    DiagnosticSeverity::Warning,
                    "GQ006",
                    DiagnosticCategory::Logic,
                    format!("Fragment '{}' is defined but never used", name),
                ));
            }
        }
        diags
    }

    /// GQ007 via AST: duplicate fields within the same selection set.
    pub(super) fn ast_check_duplicate_fields(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        file.walk(|node, _depth| {
            if node.kind() == "selection_set" {
                let mut seen: HashMap<String, u32> = HashMap::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "field" {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            if let Ok(fname) = name_node.utf8_text(source) {
                                let line = child.start_position().row as u32;
                                if let Some(&prev) = seen.get(fname) {
                                    diags.push(Diagnostic::new(
                                        lr(line),
                                        DiagnosticSeverity::Warning,
                                        "GQ007",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "Duplicate field '{}' (first at line {})",
                                            fname,
                                            prev + 1
                                        ),
                                    ));
                                } else {
                                    seen.insert(fname.to_string(), line);
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diags
    }
}
