use tree_sitter::Node;

use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::domain::type_checking::checker::TypeChecker;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::ty::Type;

impl<'a> TypeChecker<'a> {
    /// Check assignment for type consistency
    pub(super) fn check_assignment(&mut self, node: &Node) {
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");

        if let (Some(target), Some(value)) = (left, right) {
            let value_type = self.inferencer.infer_expr(&value);

            // Check for annotated assignment
            if let Some(type_node) = node.child_by_field_name("type") {
                let expected = parse_type_annotation(self.source, &type_node);

                if !self.is_assignable(&expected, &value_type) {
                    self.diagnostics.push(Diagnostic::error(
                        Range::from_node(&value),
                        "TC001",
                        DiagnosticCategory::Type,
                        format!(
                            "Type mismatch: expected '{}', got '{}'",
                            expected, value_type
                        ),
                    ));
                }
            }

            // Bind the variable
            self.inferencer.bind_assignment(&target, value_type);
        }
    }

    /// Check return statement
    pub(super) fn check_return(&mut self, node: &Node) {
        // Mark that we have a return in this function
        self.mark_has_return();

        // Get expected return type from function context
        let expected_return = self
            .current_function()
            .map(|ctx| ctx.return_type.clone())
            .unwrap_or(Type::Unknown);

        // Get actual return value type
        let actual_return = if let Some(value) = node.child(1) {
            self.inferencer.infer_expr(&value)
        } else {
            Type::None // bare "return" returns None
        };

        // Check compatibility
        if !expected_return.is_unknown() && !expected_return.is_any() {
            if !self.is_assignable(&expected_return, &actual_return) {
                self.diagnostics.push(Diagnostic::error(
                    Range::from_node(node),
                    "TC003",
                    DiagnosticCategory::Type,
                    format!(
                        "Incompatible return type: expected '{}', got '{}'",
                        expected_return, actual_return
                    ),
                ));
            }
        }
    }
}
