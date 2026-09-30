use crate::domain::syntax::parsed_file::NodeRange;
use tree_sitter::Node;

use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::type_checking::checker::{TypeChecker, VariancePosition};
use crate::domain::type_system::class_info::ClassInfo;
use crate::domain::type_system::ty::{Type, Variance};

impl<'a> TypeChecker<'a> {
    /// Validate variance usage in a generic class
    ///
    /// Covariant TypeVars should only appear in return positions (outputs)
    /// Contravariant TypeVars should only appear in parameter positions (inputs)
    pub(super) fn validate_variance_in_class(
        &mut self,
        node: &Node,
        class_name: &str,
        class_info: ClassInfo,
    ) {
        // Get the class body
        let body = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return,
        };

        // Build a map of TypeVar names to their variance
        let typevar_variance: std::collections::HashMap<String, Variance> = class_info
            .generic_params
            .iter()
            .map(|p| (p.name.clone(), p.variance))
            .collect();

        // Check each method in the class
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_definition" || child.kind() == "async_function_definition"
            {
                self.validate_method_variance(&child, class_name, &typevar_variance);
            }
        }
    }

    /// Validate variance usage in a method
    fn validate_method_variance(
        &mut self,
        node: &Node,
        _class_name: &str,
        typevar_variance: &std::collections::HashMap<String, Variance>,
    ) {
        let method_name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check parameter types (input positions - contravariant allowed)
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for param in params.children(&mut cursor) {
                if param.kind() == "typed_parameter" || param.kind() == "typed_default_parameter" {
                    if let Some(type_node) = param.child_by_field_name("type") {
                        let type_text = self.node_text(&type_node).to_string();
                        self.check_variance_position(
                            &type_node,
                            &type_text,
                            typevar_variance,
                            VariancePosition::Input,
                            &method_name,
                        );
                    }
                }
            }
        }

        // Check return type (output position - covariant allowed)
        if let Some(return_type) = node.child_by_field_name("return_type") {
            let type_text = self.node_text(&return_type).to_string();
            self.check_variance_position(
                &return_type,
                &type_text,
                typevar_variance,
                VariancePosition::Output,
                &method_name,
            );
        }
    }

    /// Check if a TypeVar is used in a valid position according to its variance
    fn check_variance_position(
        &mut self,
        node: &Node,
        type_text: &str,
        typevar_variance: &std::collections::HashMap<String, Variance>,
        position: VariancePosition,
        method_name: &str,
    ) {
        // Simple check: look for TypeVar names in the type annotation
        for (name, variance) in typevar_variance {
            // Skip if this TypeVar doesn't appear in the type
            if !type_text.contains(name) {
                continue;
            }

            let invalid = match (variance, position) {
                // Covariant TypeVar in input position is invalid
                (Variance::Covariant, VariancePosition::Input) => true,
                // Contravariant TypeVar in output position is invalid
                (Variance::Contravariant, VariancePosition::Output) => true,
                // All other combinations are valid
                _ => false,
            };

            if invalid {
                let pos_name = match position {
                    VariancePosition::Input => "parameter",
                    VariancePosition::Output => "return",
                };
                let var_name = match variance {
                    Variance::Covariant => "covariant",
                    Variance::Contravariant => "contravariant",
                    Variance::Invariant => "invariant",
                };

                self.diagnostics.push(Diagnostic::error(
                    node.to_range(),
                    "TC010",
                    DiagnosticCategory::Type,
                    format!(
                        "{} TypeVar '{}' cannot appear in {} type of method '{}'",
                        var_name, name, pos_name, method_name
                    ),
                ));
            }
        }
    }

    /// Check type arguments with variance rules
    ///
    /// For generic types, variance determines how type arguments relate:
    /// - Covariant: List[Dog] is assignable to List[Animal] (Dog <: Animal)
    /// - Contravariant: Callable[[Animal], None] is assignable to Callable[[Dog], None]
    /// - Invariant: Both types must be equal
    pub(super) fn check_type_args_with_variance(
        &self,
        class_name: &str,
        target_args: &[Type],
        source_args: &[Type],
    ) -> bool {
        // Get class info to determine variance of each type parameter
        let class_info = self.inferencer.get_class(class_name);

        for (i, (target_arg, source_arg)) in target_args.iter().zip(source_args.iter()).enumerate()
        {
            let variance = class_info
                .map(|info| info.variance_at(i))
                .unwrap_or(Variance::Invariant);

            let compatible = match variance {
                Variance::Covariant => {
                    // Source must be a subtype of target
                    // e.g., List[Dog] assignable to List[Animal] if Dog <: Animal
                    self.is_assignable(target_arg, source_arg)
                }
                Variance::Contravariant => {
                    // Target must be a subtype of source (reversed)
                    // e.g., Callable[[Animal], R] assignable to Callable[[Dog], R]
                    self.is_assignable(source_arg, target_arg)
                }
                Variance::Invariant => {
                    // Types must be equal
                    self.is_assignable(target_arg, source_arg)
                        && self.is_assignable(source_arg, target_arg)
                }
            };

            if !compatible {
                return false;
            }
        }

        true
    }
}
