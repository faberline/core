use super::JavaScriptChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl JavaScriptChecker {
    /// JS006: no-implied-eval — setTimeout/setInterval with string arg
    pub(super) fn check_implied_eval(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    let name = file.node_text(&func);
                    if name == "setTimeout" || name == "setInterval" {
                        if let Some(args) = node.child_by_field_name("arguments") {
                            if let Some(first_arg) = args.child(1) {
                                if first_arg.kind() == "string"
                                    || first_arg.kind() == "template_string"
                                {
                                    diagnostics.push(Diagnostic::error(
                                        node.to_range(),
                                        "JS006",
                                        DiagnosticCategory::Security,
                                        format!(
                                            "Implied eval — pass a function to '{}', not a string",
                                            name
                                        ),
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

    /// JS007: no-proto — detect __proto__ property access
    pub(super) fn check_no_proto(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "member_expression" {
                if let Some(prop) = node.child_by_field_name("property") {
                    if file.node_text(&prop) == "__proto__" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS007",
                            DiagnosticCategory::Style,
                            "Use Object.getPrototypeOf() instead of '__proto__'",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS008: no-with — detect with statements
    pub(super) fn check_no_with(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "with_statement" {
                diagnostics.push(Diagnostic::error(
                    node.to_range(),
                    "JS008",
                    DiagnosticCategory::Logic,
                    "'with' statement is forbidden — it makes code unpredictable",
                ));
            }
            true
        });
        diagnostics
    }

    /// JS009: no-alert — detect alert/confirm/prompt calls
    pub(super) fn check_no_alert(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "identifier" {
                        let name = file.node_text(&func);
                        if name == "alert" || name == "confirm" || name == "prompt" {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "JS009",
                                DiagnosticCategory::Style,
                                format!(
                                    "Unexpected '{}()' — use a custom UI component instead",
                                    name
                                ),
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS010: no-caller — detect arguments.caller/arguments.callee
    pub(super) fn check_no_caller(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "member_expression" {
                if let Some(obj) = node.child_by_field_name("object") {
                    if file.node_text(&obj) == "arguments" {
                        if let Some(prop) = node.child_by_field_name("property") {
                            let prop_text = file.node_text(&prop);
                            if prop_text == "caller" || prop_text == "callee" {
                                diagnostics.push(Diagnostic::error(
                                    node.to_range(),
                                    "JS010",
                                    DiagnosticCategory::Logic,
                                    format!(
                                        "'arguments.{}' is deprecated and forbidden in strict mode",
                                        prop_text
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

    /// JS011: no-extend-native — detect prototype extension of built-in objects
    pub(super) fn check_no_extend_native(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "assignment_expression" {
                if let Some(left) = node.child_by_field_name("left") {
                    let text = file.node_text(&left);
                    let builtins = ["Object", "Array", "String", "Number", "Boolean", "Function"];
                    for b in &builtins {
                        if text.starts_with(&format!("{}.prototype.", b)) {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "JS011",
                                DiagnosticCategory::Logic,
                                format!("Do not extend native '{}' prototype", b),
                            ));
                            break;
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS012: no-new-wrappers — detect new String/Number/Boolean
    pub(super) fn check_no_new_wrappers(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "new_expression" {
                if let Some(ctor) = node.child_by_field_name("constructor") {
                    let name = file.node_text(&ctor);
                    if name == "String" || name == "Number" || name == "Boolean" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS012",
                            DiagnosticCategory::Style,
                            format!(
                                "Do not use '{}' as a constructor — use a literal instead",
                                name
                            ),
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS013: no-throw-literal — detect throwing non-Error values
    pub(super) fn check_no_throw_literal(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "throw_statement" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let k = child.kind();
                    if k == "string"
                        || k == "number"
                        || k == "null"
                        || k == "undefined"
                        || k == "true"
                        || k == "false"
                        || k == "template_string"
                    {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS013",
                            DiagnosticCategory::Logic,
                            "Throw an Error object instead of a literal",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS014: no-return-assign — detect assignment inside return
    pub(super) fn check_no_return_assign(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "return_statement" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "assignment_expression" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "JS014",
                            DiagnosticCategory::Logic,
                            "Unexpected assignment in return statement",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// JS015: no-self-compare — detect x === x or x == x
    pub(super) fn check_no_self_compare(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "binary_expression" {
                if let Some(op) = node.child_by_field_name("operator") {
                    let op_text = file.node_text(&op);
                    if op_text == "==" || op_text == "===" || op_text == "!=" || op_text == "!==" {
                        if let (Some(left), Some(right)) = (
                            node.child_by_field_name("left"),
                            node.child_by_field_name("right"),
                        ) {
                            if file.node_text(&left) == file.node_text(&right) {
                                diagnostics.push(Diagnostic::warning(
                                    node.to_range(),
                                    "JS015",
                                    DiagnosticCategory::Logic,
                                    "Comparing a value to itself is always redundant",
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
}
