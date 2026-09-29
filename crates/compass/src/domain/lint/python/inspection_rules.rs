use std::collections::HashMap;

use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range, TextEdit};
use crate::syntax::ParsedFile;

impl PythonChecker {
    /// Check for accessing private members (starting with _) from outside
    pub(super) fn check_private_member_access(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            // Check attribute access like obj._private
            if node.kind() == "attribute" {
                if let Some(attr) = node.child_by_field_name("attribute") {
                    let attr_name = file.node_text(&attr);
                    // Check if it starts with _ but not __ (dunder methods are OK)
                    if attr_name.starts_with('_') && !attr_name.starts_with("__") {
                        // Check if this is not self._private or cls._private
                        if let Some(obj) = node.child_by_field_name("object") {
                            let obj_name = file.node_text(&obj);
                            if obj_name != "self" && obj_name != "cls" {
                                diagnostics.push(Diagnostic::warning(
                                    Range::from_node(node),
                                    "PY402",
                                    DiagnosticCategory::Style,
                                    format!(
                                        "Accessing private member '{}' from outside class",
                                        attr_name
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

    /// Check for duplicate dictionary keys
    pub(super) fn check_duplicate_dict_keys(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "dictionary" {
                let mut seen_keys: HashMap<String, Range> = HashMap::new();
                let mut cursor = node.walk();

                for child in node.children(&mut cursor) {
                    if child.kind() == "pair" {
                        if let Some(key) = child.child_by_field_name("key") {
                            // Only check literal keys (strings, numbers)
                            let key_text = file.node_text(&key);
                            if matches!(key.kind(), "string" | "integer" | "float" | "identifier") {
                                if let Some(prev_range) = seen_keys.get(key_text) {
                                    diagnostics.push(Diagnostic::warning(
                                        Range::from_node(&key),
                                        "PY403",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "Duplicate dictionary key '{}' (first defined at line {})",
                                            key_text,
                                            prev_range.start.line + 1
                                        ),
                                    ));
                                } else {
                                    seen_keys.insert(key_text.to_string(), Range::from_node(&key));
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

    /// Check for boolean comparisons that can be simplified
    /// e.g., `if x == True` -> `if x`, `if x == False` -> `if not x`
    pub(super) fn check_simplify_boolean(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "comparison_operator" {
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();

                // Look for patterns like `x == True` or `x == False`
                for (i, child) in children.iter().enumerate() {
                    if child.kind() == "==" || child.kind() == "!=" {
                        // Check the operand after the operator
                        if let Some(right) = children.get(i + 1) {
                            let right_text = file.node_text(right);
                            if right_text == "True" || right_text == "False" {
                                let suggestion = if right_text == "True" {
                                    if child.kind() == "==" {
                                        "Use 'if x' instead of 'if x == True'"
                                    } else {
                                        "Use 'if not x' instead of 'if x != True'"
                                    }
                                } else if child.kind() == "==" {
                                    "Use 'if not x' instead of 'if x == False'"
                                } else {
                                    "Use 'if x' instead of 'if x != False'"
                                };

                                diagnostics.push(Diagnostic::new(
                                    Range::from_node(node),
                                    crate::diagnostic::DiagnosticSeverity::Hint,
                                    "PY404",
                                    DiagnosticCategory::Style,
                                    suggestion,
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

    /// Check for `== None` instead of `is None`
    pub(super) fn check_none_comparison(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "comparison_operator" {
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();

                for (i, child) in children.iter().enumerate() {
                    if child.kind() == "==" || child.kind() == "!=" {
                        // Check if comparing with None on right: x == None
                        if let Some(right) = children.get(i + 1) {
                            if file.node_text(right) == "None" {
                                if let Some(left) = children.get(i - 1) {
                                    let left_text = file.node_text(left);
                                    let (suggestion, fix_text) = if child.kind() == "==" {
                                        (
                                            "Use 'is None' instead of '== None'",
                                            format!("{} is None", left_text),
                                        )
                                    } else {
                                        (
                                            "Use 'is not None' instead of '!= None'",
                                            format!("{} is not None", left_text),
                                        )
                                    };

                                    let range = Range::from_node(node);
                                    diagnostics.push(
                                        Diagnostic::warning(
                                            range.clone(),
                                            "PY405",
                                            DiagnosticCategory::Style,
                                            suggestion,
                                        )
                                        .with_fix(
                                            "Replace with identity check",
                                            vec![TextEdit {
                                                range,
                                                new_text: fix_text,
                                            }],
                                        ),
                                    );
                                }
                            }
                        }
                        // Check left side: None == x
                        if i > 0 {
                            if let Some(left) = children.get(i - 1) {
                                if file.node_text(left) == "None" {
                                    if let Some(right) = children.get(i + 1) {
                                        let right_text = file.node_text(right);
                                        let (suggestion, fix_text) = if child.kind() == "==" {
                                            (
                                                "Use 'is None' instead of 'None =='",
                                                format!("{} is None", right_text),
                                            )
                                        } else {
                                            (
                                                "Use 'is not None' instead of 'None !='",
                                                format!("{} is not None", right_text),
                                            )
                                        };

                                        let range = Range::from_node(node);
                                        diagnostics.push(
                                            Diagnostic::warning(
                                                range.clone(),
                                                "PY405",
                                                DiagnosticCategory::Style,
                                                suggestion,
                                            )
                                            .with_fix(
                                                "Replace with identity check",
                                                vec![TextEdit {
                                                    range,
                                                    new_text: fix_text,
                                                }],
                                            ),
                                        );
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

    /// Check for statements that have no effect
    pub(super) fn check_statement_no_effect(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "expression_statement" {
                if let Some(expr) = node.child(0) {
                    // These expression types have no effect on their own
                    let is_useless = match expr.kind() {
                        // Literals (but strings might be docstrings)
                        // Skip ellipsis (...) as it's used as placeholder in stubs/abstract methods
                        "integer" | "float" | "true" | "false" | "none" => true,
                        "ellipsis" => false, // ... is intentional placeholder
                        // String literals - check if docstring
                        "string" => !Self::is_docstring(node, file),
                        // Identifiers (just referencing a variable)
                        "identifier" => true,
                        // Attribute access without assignment
                        "attribute" => true,
                        // Subscript access without assignment
                        "subscript" => true,
                        // Binary operations that don't assign
                        "binary_operator" => {
                            // Exclude augmented assignments
                            let text = file.node_text(&expr);
                            !text.contains("+=") && !text.contains("-=")
                        }
                        // Comparison that's not used
                        "comparison_operator" => true,
                        _ => false,
                    };

                    if is_useless {
                        diagnostics.push(Diagnostic::warning(
                            Range::from_node(node),
                            "PY406",
                            DiagnosticCategory::Logic,
                            "Statement has no effect",
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// Check if a string expression_statement is a docstring
    fn is_docstring(node: &tree_sitter::Node<'_>, _file: &ParsedFile) -> bool {
        if let Some(parent) = node.parent() {
            // Docstrings appear as first statement in module, class, or function
            match parent.kind() {
                "module" | "block" => {
                    // Check if this is the first statement (or first after decorators)
                    let mut cursor = parent.walk();
                    for child in parent.children(&mut cursor) {
                        // Skip decorators and comments
                        if matches!(child.kind(), "decorator" | "comment") {
                            continue;
                        }
                        // If this node is the first expression_statement, it's a docstring
                        return child.id() == node.id();
                    }
                }
                _ => {}
            }
        }
        false
    }
}
