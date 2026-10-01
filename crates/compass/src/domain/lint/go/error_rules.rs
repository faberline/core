use super::GoChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl GoChecker {
    /// GO001: Unchecked error — assignment with `_` for error return
    pub(super) fn check_unchecked_error(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "short_var_declaration" {
                // Check left side for blank identifiers in error position
                if let Some(left) = node.child_by_field_name("left") {
                    let mut cursor = left.walk();
                    let children: Vec<_> = left.children(&mut cursor).collect();

                    // If last identifier on left is `_`, likely ignoring error
                    if children.len() >= 2 {
                        if let Some(last) = children.iter().rev()
                            .find(|c| c.kind() == "identifier" || c.kind() == "blank_identifier")
                        {
                            if last.kind() == "blank_identifier"
                                || file.node_text(last) == "_"
                            {
                                // Verify the right side is a call expression (likely returns error)
                                if let Some(right) = node.child_by_field_name("right") {
                                    let mut rc = right.walk();
                                    let right_children: Vec<_> = right.children(&mut rc).collect();
                                    let has_call = right_children.iter()
                                        .any(|c| c.kind() == "call_expression");
                                    if has_call {
                                        diagnostics.push(Diagnostic::warning(
                                            node.to_range(),
                                            "GO001",
                                            DiagnosticCategory::Logic,
                                            "Error return value is discarded with '_' — handle the error",
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

    /// GO006: Empty error handling branch (if err != nil { } with empty block)
    pub(super) fn check_empty_error_handling(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "if_statement" {
                if let Some(condition) = node.child_by_field_name("condition") {
                    let cond_text = file.node_text(&condition);
                    if cond_text.contains("err") && cond_text.contains("nil") {
                        // Check if the consequence block is empty
                        if let Some(body) = node.child_by_field_name("consequence") {
                            let mut cursor = body.walk();
                            let non_brace = body.children(&mut cursor)
                                .filter(|c| c.kind() != "{" && c.kind() != "}")
                                .count();
                            if non_brace == 0 {
                                diagnostics.push(Diagnostic::warning(
                                    node.to_range(),
                                    "GO006",
                                    DiagnosticCategory::Logic,
                                    "Empty error handling block — handle the error or add a comment",
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

    /// GO007: fmt.Sprintf in error string (use fmt.Errorf instead)
    pub(super) fn check_sprintf_in_error(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    let func_text = file.node_text(&func);
                    if func_text == "errors.New" || func_text == "fmt.Errorf" {
                        // Check if argument is fmt.Sprintf(...)
                        if let Some(args) = node.child_by_field_name("arguments") {
                            let mut cursor = args.walk();
                            for child in args.children(&mut cursor) {
                                if child.kind() == "call_expression" {
                                    if let Some(inner_func) = child.child_by_field_name("function") {
                                        if file.node_text(&inner_func) == "fmt.Sprintf" {
                                            diagnostics.push(Diagnostic::warning(
                                                node.to_range(),
                                                "GO007",
                                                DiagnosticCategory::Style,
                                                "Use fmt.Errorf() directly instead of errors.New(fmt.Sprintf(...))",
                                            ));
                                        }
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
}
