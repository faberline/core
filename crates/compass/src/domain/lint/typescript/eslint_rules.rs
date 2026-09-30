use super::TypeScriptChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl TypeScriptChecker {
    /// TS006: no-floating-promises — expression statement with call but no await
    pub(super) fn check_floating_promises(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "expression_statement" {
                if let Some(child) = node.child(0) {
                    if child.kind() == "call_expression" {
                        let text = file.node_text(&child);
                        if text.contains("async") || text.ends_with("Async()") {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "TS006",
                                DiagnosticCategory::Logic,
                                "Floating promise — add 'await' or handle the returned promise",
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS007: strict-boolean-expressions — if condition is a bare identifier
    pub(super) fn check_strict_boolean(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "if_statement" {
                if let Some(cond) = node.child_by_field_name("condition") {
                    let inner = if cond.kind() == "parenthesized_expression" {
                        cond.child(1)
                    } else {
                        Some(cond)
                    };
                    if let Some(expr) = inner {
                        if expr.kind() == "identifier" {
                            diagnostics.push(Diagnostic::new(
                                expr.to_range(),
                                DiagnosticSeverity::Information,
                                "TS007",
                                DiagnosticCategory::Type,
                                "Non-boolean used in condition — use an explicit comparison",
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS008: no-unnecessary-type-assertion — `x as T` where x and T look identical
    pub(super) fn check_unnecessary_type_assertion(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "as_expression" {
                if let (Some(expr), Some(ty)) = (node.child(0), node.child(2)) {
                    let expr_text = file.node_text(&expr).trim().to_string();
                    let ty_text = file.node_text(&ty).trim().to_string();
                    if expr_text == ty_text {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "TS008",
                            DiagnosticCategory::Type,
                            "Unnecessary type assertion — expression already has this type",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS009: no-empty-interface — interface with no members
    pub(super) fn check_empty_interface(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "interface_declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "interface_body" || child.kind() == "object_type" {
                        let mut inner = child.walk();
                        let has_members = child.children(&mut inner).any(|c| {
                            c.kind() != "{" && c.kind() != "}" && !c.kind().contains("comment")
                        });
                        if !has_members {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "TS009",
                                DiagnosticCategory::Style,
                                "Empty interface — use a type alias or add members",
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS010: no-duplicate-enum-values
    pub(super) fn check_duplicate_enum_values(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "enum_declaration" {
                let mut seen = std::collections::HashMap::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "enum_body" {
                        let mut inner = child.walk();
                        for member in child.children(&mut inner) {
                            if let Some(val) = member.child_by_field_name("value") {
                                let val_text = file.node_text(&val).trim().to_string();
                                if let Some(prev_line) = seen.get(&val_text) {
                                    diagnostics.push(Diagnostic::warning(
                                        val.to_range(),
                                        "TS010",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "Duplicate enum value '{}' (first at line {})",
                                            val_text, prev_line
                                        ),
                                    ));
                                } else {
                                    seen.insert(val_text, val.start_position().row + 1);
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS011: prefer-optional-chain — detect `foo && foo.bar`
    pub(super) fn check_prefer_optional_chain(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "binary_expression" {
                if let Some(op) = node.child_by_field_name("operator") {
                    if file.node_text(&op) == "&&" {
                        if let (Some(left), Some(right)) = (
                            node.child_by_field_name("left"),
                            node.child_by_field_name("right"),
                        ) {
                            if left.kind() == "identifier" && right.kind() == "member_expression" {
                                if let Some(obj) = right.child_by_field_name("object") {
                                    let left_text = file.node_text(&left);
                                    let obj_text = file.node_text(&obj);
                                    if left_text == obj_text {
                                        diagnostics.push(Diagnostic::new(
                                            node.to_range(),
                                            DiagnosticSeverity::Hint,
                                            "TS011",
                                            DiagnosticCategory::Style,
                                            "Prefer optional chaining (?.) over '&&' guard",
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS012: no-namespace — detect namespace/module declarations
    pub(super) fn check_no_namespace(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "ambient_declaration"
                || node.kind() == "module"
                || node.kind() == "internal_module"
            {
                let text = file.node_text(node);
                if text.starts_with("namespace") || text.starts_with("module") {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        "TS012",
                        DiagnosticCategory::Style,
                        "Avoid namespace/module declarations — use ES modules instead",
                    ));
                }
            }
            true
        });
        diagnostics
    }

    /// TS013: explicit-function-return-type
    pub(super) fn check_explicit_return_type(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "function_declaration" {
                let mut ret_cursor = node.walk();
                let has_return_type = node
                    .children(&mut ret_cursor)
                    .any(|c| c.kind() == "type_annotation");
                if !has_return_type {
                    diagnostics.push(Diagnostic::new(
                        node.to_range(),
                        DiagnosticSeverity::Information,
                        "TS013",
                        DiagnosticCategory::Type,
                        "Function missing explicit return type annotation",
                    ));
                }
            }
            true
        });
        diagnostics
    }

    /// TS014: no-var-requires — detect require() calls
    pub(super) fn check_no_var_requires(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "identifier" && file.node_text(&func) == "require" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "TS014",
                            DiagnosticCategory::Style,
                            "Use ES 'import' instead of 'require()'",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// TS015: consistent-type-imports — import of uppercase names could be type-only
    pub(super) fn check_consistent_type_imports(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "import_statement" {
                let text = file.node_text(node);
                if text.contains("import type") || text.contains("import {type") {
                    return true;
                }
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "import_clause" {
                        let mut inner = child.walk();
                        for spec in child.children(&mut inner) {
                            if spec.kind() == "named_imports" {
                                let mut spec_cur = spec.walk();
                                let all_upper = spec
                                    .children(&mut spec_cur)
                                    .filter(|c| c.kind() == "import_specifier")
                                    .all(|c| {
                                        let n = file.node_text(&c).trim().to_string();
                                        n.chars().next().map_or(false, |ch| ch.is_uppercase())
                                    });
                                let has_any = spec
                                    .children(&mut spec.walk())
                                    .any(|c| c.kind() == "import_specifier");
                                if has_any && all_upper {
                                    diagnostics.push(Diagnostic::new(
                                        node.to_range(),
                                        DiagnosticSeverity::Hint,
                                        "TS015",
                                        DiagnosticCategory::Style,
                                        "All imported names are types — use 'import type' instead",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }
}
