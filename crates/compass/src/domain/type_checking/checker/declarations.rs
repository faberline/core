use crate::domain::syntax::parsed_file::NodeRange;
use tree_sitter::Node;

use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::domain::type_checking::checker::{FunctionContext, TypeChecker};
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::ty::Type;

impl<'a> TypeChecker<'a> {
    /// Recursively check a node
    pub(super) fn check_node(&mut self, node: &Node) {
        match node.kind() {
            "class_definition" => {
                self.check_class(node);
                return; // check_class handles its own recursion
            }
            "function_definition" | "async_function_definition" => {
                self.check_function(node);
                return; // check_function handles its own recursion
            }
            "if_statement" => {
                self.check_if_statement(node);
                return; // check_if_statement handles its own recursion
            }
            "while_statement" => {
                self.check_while_statement(node);
                return; // handles its own recursion
            }
            "for_statement" => {
                self.check_for_statement(node);
                return; // handles its own recursion
            }
            "try_statement" => {
                self.check_try_statement(node);
                return; // handles its own recursion
            }
            "import_statement" | "import_from_statement" => {
                self.inferencer.analyze_import(node);
            }
            "assignment" => {
                self.check_assignment(node);
            }
            "return_statement" => {
                self.check_return(node);
            }
            "call" => {
                self.check_call(node);
            }
            "attribute" => {
                self.check_attribute(node);
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(&child);
        }
    }

    /// Check function definition
    fn check_function(&mut self, node: &Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Get return type annotation
        let return_type = node
            .child_by_field_name("return_type")
            .map(|rt| parse_type_annotation(self.source, &rt))
            .unwrap_or(Type::Unknown);

        // Check for missing return type annotation (only for public functions)
        if node.child_by_field_name("return_type").is_none() && !name.starts_with('_') {
            self.diagnostics.push(Diagnostic::new(
                node.to_range(),
                DiagnosticSeverity::Hint,
                "TC002",
                DiagnosticCategory::Type,
                format!("Function '{}' is missing return type annotation", name),
            ));
        }

        // Analyze function and add to environment
        self.inferencer.analyze_function(node);

        // Push function context
        self.function_stack.push(FunctionContext {
            name: name.clone(),
            return_type: return_type.clone(),
            has_return: false,
        });

        // Check function body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.check_node(&child);
            }
        }

        // Pop function context and check for missing return
        if let Some(ctx) = self.function_stack.pop() {
            // If function has declared return type (not None/Unknown) but no return
            if !ctx.has_return && !matches!(ctx.return_type, Type::None | Type::Unknown | Type::Any)
            {
                self.diagnostics.push(Diagnostic::warning(
                    node.to_range(),
                    "TC003",
                    DiagnosticCategory::Type,
                    format!(
                        "Function '{}' declares return type '{}' but may not return a value",
                        ctx.name, ctx.return_type
                    ),
                ));
            }
        }
    }

    /// Check class definition
    fn check_class(&mut self, node: &Node) {
        let class_name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Analyze class and register in inferencer
        self.inferencer.analyze_class(node);

        // Validate variance usage in class methods
        if let Some(class_info) = self.inferencer.get_class(&class_name) {
            if class_info.is_generic() {
                self.validate_variance_in_class(node, &class_name, class_info.clone());
            }
        }

        // Check class body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.check_node(&child);
            }
        }
    }
}
