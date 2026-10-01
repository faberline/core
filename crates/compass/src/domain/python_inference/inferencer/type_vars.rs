use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::python_inference::type_var_info::TypeVarInfo;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::ty::{Type, Variance};

impl<'a> TypeInferencer<'a> {
    /// Parse a TypeVar(...) call and extract variance, bounds, and constraints
    ///
    /// Handles patterns like:
    /// - `TypeVar("T")`
    /// - `TypeVar("T", covariant=True)`
    /// - `TypeVar("T", contravariant=True)`
    /// - `TypeVar("T", bound=SomeType)`
    /// - `TypeVar("T", int, str)` (constraints)
    pub fn parse_typevar_call(&self, node: &Node) -> Option<TypeVarInfo> {
        // Must be a call expression
        if node.kind() != "call" {
            return None;
        }

        // Get function being called
        let func = node.child_by_field_name("function")?;
        let func_name = self.node_text(&func);

        // Must be a TypeVar call
        if func_name != "TypeVar" {
            return None;
        }

        // Get arguments
        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();

        let mut name: Option<String> = None;
        let mut variance = Variance::Invariant;
        let mut bound: Option<Type> = None;
        let mut constraints: Vec<Type> = Vec::new();
        let mut positional_count = 0;

        for child in args.children(&mut cursor) {
            match child.kind() {
                "string" => {
                    // First string is the TypeVar name
                    if name.is_none() {
                        let text = self.node_text(&child);
                        // Remove quotes
                        let text = text.trim_matches(|c| c == '"' || c == '\'');
                        name = Some(text.to_string());
                    }
                    positional_count += 1;
                }
                "identifier" | "subscript" | "attribute" => {
                    // Positional type constraints (not the first string name)
                    if positional_count > 0 {
                        let ty = parse_type_annotation(self.source, &child);
                        constraints.push(ty);
                    }
                    positional_count += 1;
                }
                "keyword_argument" => {
                    // Parse keyword arguments: covariant, contravariant, bound
                    if let Some(arg_name) = child.child_by_field_name("name") {
                        let key = self.node_text(&arg_name);
                        if let Some(value_node) = child.child_by_field_name("value") {
                            let value = self.node_text(&value_node);
                            match key {
                                "covariant" if value == "True" => {
                                    variance = Variance::Covariant;
                                }
                                "contravariant" if value == "True" => {
                                    variance = Variance::Contravariant;
                                }
                                "bound" => {
                                    bound = Some(parse_type_annotation(self.source, &value_node));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Some(TypeVarInfo {
            name: name?,
            variance,
            bound,
            constraints,
        })
    }

    /// Handle an assignment that might be a TypeVar declaration
    /// Returns true if a TypeVar was registered
    pub fn try_register_typevar_assignment(&mut self, node: &Node) -> bool {
        // Pattern: T = TypeVar("T", ...)
        if node.kind() != "assignment" {
            return false;
        }

        let left = match node.child_by_field_name("left") {
            Some(l) if l.kind() == "identifier" => l,
            _ => return false,
        };

        let right = match node.child_by_field_name("right") {
            Some(r) if r.kind() == "call" => r,
            _ => return false,
        };

        let var_name = self.node_text(&left).to_string();

        if let Some(info) = self.parse_typevar_call(&right) {
            // Register the TypeVar with extracted variance
            self.register_type_var_with_variance(
                &var_name,
                info.bound,
                info.constraints,
                info.variance,
            );
            return true;
        }

        false
    }
}
