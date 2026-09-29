use super::GoChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Range};
use crate::syntax::ParsedFile;

impl GoChecker {
    /// GO003: Shadowed variable — variable redeclared in inner scope with same name
    pub(super) fn check_shadowed_variable(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut scope_stack: Vec<Vec<String>> = vec![Vec::new()];

        file.walk(|node, _depth| {
            match node.kind() {
                "function_declaration"
                | "method_declaration"
                | "func_literal"
                | "if_statement"
                | "for_statement"
                | "block" => {
                    scope_stack.push(Vec::new());
                }
                "short_var_declaration" => {
                    if let Some(left) = node.child_by_field_name("left") {
                        let mut cursor = left.walk();
                        for child in left.children(&mut cursor) {
                            if child.kind() == "identifier" {
                                let name = file.node_text(&child).to_string();
                                // Check if name exists in any outer scope
                                let shadowed = scope_stack
                                    .iter()
                                    .rev()
                                    .skip(1)
                                    .any(|scope| scope.contains(&name));
                                if shadowed {
                                    diagnostics.push(Diagnostic::warning(
                                        Range::from_node(&child),
                                        "GO003",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "Variable '{}' shadows a variable in an outer scope",
                                            name
                                        ),
                                    ));
                                }
                                if let Some(current) = scope_stack.last_mut() {
                                    current.push(name);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            true
        });

        diagnostics
    }

    /// GO004: Naked return in function with named return values
    pub(super) fn check_naked_return(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
                // Check if function has named return values
                let has_named_returns = self.has_named_returns(node, file);
                if has_named_returns {
                    // Walk function body for naked returns
                    if let Some(body) = node.child_by_field_name("body") {
                        self.find_naked_returns(&body, file, &mut diagnostics);
                    }
                }
            }
            true
        });

        diagnostics
    }

    fn has_named_returns(&self, func_node: &tree_sitter::Node<'_>, _file: &ParsedFile) -> bool {
        if let Some(result) = func_node.child_by_field_name("result") {
            // Named returns use parameter_list with identifiers
            if result.kind() == "parameter_list" {
                let mut cursor = result.walk();
                for child in result.children(&mut cursor) {
                    if child.kind() == "parameter_declaration" {
                        // Named return has both name and type
                        if child.child_by_field_name("name").is_some() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn find_naked_returns(
        &self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "return_statement" {
                // Naked return has no expression list
                let mut rc = child.walk();
                let has_values = child
                    .children(&mut rc)
                    .any(|c| c.kind() == "expression_list");
                if !has_values {
                    let text = file.node_text(&child).trim();
                    if text == "return" {
                        diagnostics.push(Diagnostic::warning(
                            Range::from_node(&child),
                            "GO004",
                            DiagnosticCategory::Style,
                            "Naked return in function with named return values — consider explicit return",
                        ));
                    }
                }
            } else if child.kind() != "func_literal" && child.kind() != "function_declaration" {
                // Recurse but not into nested functions
                self.find_naked_returns(&child, file, diagnostics);
            }
        }
    }

    /// GO005: context.Background() used outside main/init function
    pub(super) fn check_context_background(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut current_func_name: Option<String> = None;

        file.walk(|node, _depth| {
            match node.kind() {
                "function_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        current_func_name = Some(file.node_text(&name_node).to_string());
                    }
                }
                "call_expression" => {
                    if let Some(func) = node.child_by_field_name("function") {
                        let text = file.node_text(&func);
                        if text == "context.Background" || text == "context.TODO" {
                            let in_allowed = current_func_name.as_deref()
                                .map(|n| n == "main" || n == "init" || n == "TestMain")
                                .unwrap_or(false);
                            if !in_allowed {
                                diagnostics.push(Diagnostic::warning(
                                    Range::from_node(node),
                                    "GO005",
                                    DiagnosticCategory::Logic,
                                    format!(
                                        "{}() used outside main/init — prefer passing context as parameter",
                                        text
                                    ),
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            true
        });

        diagnostics
    }

    /// GO008: Exported function/type missing doc comment
    pub(super) fn check_exported_doc_comment(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            let name_field = match node.kind() {
                "function_declaration" | "method_declaration" | "type_declaration" => {
                    node.child_by_field_name("name")
                }
                "type_spec" => node.child_by_field_name("name"),
                _ => None,
            };

            if let Some(name_node) = name_field {
                let name = file.node_text(&name_node);
                // Exported names start with uppercase
                if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    let has_doc = self.has_preceding_comment(node, file);
                    if !has_doc {
                        diagnostics.push(Diagnostic::new(
                            Range::from_node(&name_node),
                            DiagnosticSeverity::Information,
                            "GO008",
                            DiagnosticCategory::Style,
                            format!("Exported name '{}' should have a doc comment", name),
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    fn has_preceding_comment(&self, node: &tree_sitter::Node<'_>, file: &ParsedFile) -> bool {
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "comment" {
                let text = file.node_text(&prev);
                // Go doc comments start with // followed by the name
                return text.starts_with("//");
            }
        }
        false
    }

    /// GO009: Goroutine spawned without a WaitGroup or context cancellation check
    ///
    /// Flags `go func()` literals where neither a `sync.WaitGroup` Add/Done
    /// pattern nor a context parameter is visible in the enclosing function
    /// signature.  This is a conservative heuristic — it fires when the word
    /// "wg" or "ctx" is absent from the enclosing function body.
    pub(super) fn check_goroutine_leak(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "go_statement" {
                // Look for `go func()` — anonymous goroutine literal
                if let Some(call) = node.child_by_field_name("call") {
                    if let Some(func) = call.child_by_field_name("function") {
                        if func.kind() == "func_literal" {
                            let body_text = file.node_text(node);
                            // Heuristic: flag if neither WaitGroup helpers nor
                            // context cancellation channels are referenced.
                            let has_wg = body_text.contains("wg.")
                                || body_text.contains(".Add(")
                                || body_text.contains(".Done()");
                            let has_ctx = body_text.contains("ctx.Done()")
                                || body_text.contains("<-ctx");
                            if !has_wg && !has_ctx {
                                diagnostics.push(Diagnostic::new(
                                    Range::from_node(node),
                                    DiagnosticSeverity::Warning,
                                    "GO009",
                                    DiagnosticCategory::Logic,
                                    "Goroutine spawned without WaitGroup or context cancellation — potential goroutine leak",
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
