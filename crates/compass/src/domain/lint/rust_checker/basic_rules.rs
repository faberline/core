use super::RustChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl RustChecker {
    /// Check for unsafe blocks
    pub(super) fn check_unsafe_blocks(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "unsafe_block" {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "RS201",
                    DiagnosticCategory::Security,
                    "Unsafe block - ensure memory safety is manually verified",
                ));
            }
            true
        });

        diagnostics
    }

    /// Check for .clone() calls that might be unnecessary
    pub(super) fn check_clone_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "field_expression" {
                        if let Some(field) = func.child_by_field_name("field") {
                            if file.node_text(&field) == "clone" {
                                diagnostics.push(Diagnostic::new(
                                    node.to_range(),
                                    DiagnosticSeverity::Hint,
                                    "RS001",
                                    DiagnosticCategory::Style,
                                    "Consider if .clone() is necessary - borrowing may be more efficient",
                                ));
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// Check for unwrap() calls
    pub(super) fn check_unwrap(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "field_expression" {
                        if let Some(field) = func.child_by_field_name("field") {
                            let field_name = file.node_text(&field);
                            if field_name == "unwrap" || field_name == "expect" {
                                diagnostics.push(Diagnostic::warning(
                                    node.to_range(),
                                    "RS101",
                                    DiagnosticCategory::Logic,
                                    format!(
                                        ".{}() can panic - consider using ? or match for error handling",
                                        field_name
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// RS102: todo!/unimplemented! macros
    pub(super) fn check_todo_macros(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "macro_invocation" {
                if let Some(macro_node) = node.child_by_field_name("macro") {
                    let macro_name = file.node_text(&macro_node);
                    if macro_name == "todo" || macro_name == "unimplemented" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "RS102",
                            DiagnosticCategory::Logic,
                            format!("{}! macro will panic at runtime", macro_name),
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// RS103: dbg! macro left in code
    pub(super) fn check_dbg_macro(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "macro_invocation" {
                if let Some(macro_node) = node.child_by_field_name("macro") {
                    if file.node_text(&macro_node) == "dbg" {
                        diagnostics.push(Diagnostic::new(
                            node.to_range(),
                            DiagnosticSeverity::Hint,
                            "RS103",
                            DiagnosticCategory::Style,
                            "dbg! macro should be removed in production",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }
}
