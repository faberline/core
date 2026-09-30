use super::JavaScriptChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl JavaScriptChecker {
    /// JS001: Detect `var` declarations and suggest `let` or `const`
    ///
    /// The `var` keyword has function scope and hoisting behavior that
    /// can lead to subtle bugs. Modern JavaScript should use `let` or `const`.
    pub(super) fn check_var_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "variable_declaration" {
                let text = file.node_text(node);
                if text.starts_with("var ") {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        "JS001",
                        DiagnosticCategory::Style,
                        "Avoid 'var' — use 'let' or 'const' for block scoping",
                    ));
                }
            }
            true
        });

        diagnostics
    }

    /// JS002: Detect `console.log` statements left in code
    ///
    /// Console statements should be removed before production deployment.
    pub(super) fn check_console_log(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "member_expression" {
                        let text = file.node_text(&func);
                        if text == "console.log" {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "JS002",
                                DiagnosticCategory::Style,
                                "Remove 'console.log' before production",
                            ));
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// JS003: Detect loose equality (`==` / `!=`) and suggest strict equality
    ///
    /// Loose equality performs type coercion which can lead to unexpected
    /// results. Use `===` and `!==` for predictable comparisons.
    pub(super) fn check_loose_equality(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "binary_expression" {
                if let Some(op) = node.child_by_field_name("operator") {
                    let op_text = file.node_text(&op);
                    if op_text == "==" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS003",
                            DiagnosticCategory::Logic,
                            "Use '===' instead of '==' to avoid type coercion",
                        ));
                    } else if op_text == "!=" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS003",
                            DiagnosticCategory::Logic,
                            "Use '!==' instead of '!=' to avoid type coercion",
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// JS004: Detect `eval()` usage (security risk)
    ///
    /// `eval()` executes arbitrary strings as code, creating security
    /// vulnerabilities and making code harder to analyze.
    pub(super) fn check_eval_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "identifier" {
                        let name = file.node_text(&func);
                        if name == "eval" {
                            diagnostics.push(Diagnostic::new(
                                node.to_range(),
                                DiagnosticSeverity::Error,
                                "JS004",
                                DiagnosticCategory::Security,
                                "Avoid 'eval()' — it executes arbitrary code and is a security risk",
                            ));
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// JS005: Detect `debugger` statements left in code
    pub(super) fn check_debugger_statement(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "debugger_statement" {
                diagnostics.push(Diagnostic::warning(
                    node.to_range(),
                    "JS005",
                    DiagnosticCategory::Logic,
                    "Remove 'debugger' statement before production",
                ));
            }
            true
        });
        diagnostics
    }
}
