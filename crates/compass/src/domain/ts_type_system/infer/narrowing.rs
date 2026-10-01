use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::ts_type_system::infer::TsTypeInferencer;
use crate::domain::ts_type_system::types::is_assignable_to;
use crate::type_inference::Type;

impl<'a> TsTypeInferencer<'a> {
    /// Apply type guard narrowing
    pub fn apply_type_guard(&mut self, condition: &Node, is_true_branch: bool) {
        match condition.kind() {
            "binary_expression" => {
                let left = condition.child_by_field_name("left");
                let op = condition.child_by_field_name("operator");
                let right = condition.child_by_field_name("right");

                let op_text = op.map(|o| self.node_text(&o)).unwrap_or("");

                match op_text {
                    // typeof narrowing
                    "===" | "==" if is_true_branch => {
                        self.apply_typeof_guard(&left, &right);
                    }
                    "!==" | "!=" if !is_true_branch => {
                        self.apply_typeof_guard(&left, &right);
                    }
                    // instanceof narrowing
                    "instanceof" => {
                        if let Some(l) = left {
                            if l.kind() == "identifier" {
                                let var_name = self.node_text(&l);
                                if let Some(r) = right {
                                    let class_name = self.node_text(&r);
                                    if is_true_branch {
                                        let ty = Type::Instance {
                                            name: class_name.to_string(),
                                            module: None,
                                            type_args: vec![],
                                        };
                                        self.narrowed_types.insert(var_name.to_string(), ty);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            "unary_expression" => {
                let op = condition.child_by_field_name("operator");
                if op.map(|o| self.node_text(&o)) == Some("!") {
                    if let Some(arg) = condition.child_by_field_name("argument") {
                        // Invert the guard
                        self.apply_type_guard(&arg, !is_true_branch);
                    }
                }
            }
            _ => {}
        }
    }

    /// Apply typeof guard
    fn apply_typeof_guard(&mut self, typeof_side: &Option<Node>, literal_side: &Option<Node>) {
        // typeof x === "string"
        if let (Some(typeof_node), Some(literal)) = (typeof_side, literal_side) {
            if typeof_node.kind() == "unary_expression" {
                let op = typeof_node.child_by_field_name("operator");
                if op.map(|o| self.node_text(&o)) == Some("typeof") {
                    if let Some(arg) = typeof_node.child_by_field_name("argument") {
                        if arg.kind() == "identifier" {
                            let var_name = self.node_text(&arg);
                            let type_str = self
                                .node_text(literal)
                                .trim_matches(|c| c == '"' || c == '\'');

                            let narrowed = match type_str {
                                "string" => Type::Str,
                                "number" => Type::Float,
                                "boolean" => Type::Bool,
                                "undefined" => Type::None,
                                "object" => Type::Any,
                                "function" => Type::Any,
                                _ => return,
                            };

                            self.narrowed_types.insert(var_name.to_string(), narrowed);
                        }
                    }
                }
            }
        }
    }

    /// Check structural compatibility between a value and an interface
    pub fn check_structural_compatibility(&self, value_ty: &Type, interface_name: &str) -> bool {
        let interface = match self.context.interfaces.get(interface_name) {
            Some(i) => i,
            None => return false,
        };

        // Get value members
        let value_members: HashMap<String, Type> = match value_ty {
            Type::Protocol { members, .. } => members.iter().cloned().collect(),
            Type::Instance { name, .. } => {
                if let Some(class) = self.context.classes.get(name) {
                    class
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), v.ty.clone()))
                        .chain(class.methods.iter().map(|(k, v)| (k.clone(), v.clone())))
                        .collect()
                } else {
                    return false;
                }
            }
            _ => return false,
        };

        // Check required properties
        for (prop_name, prop_ty) in &interface.properties {
            match value_members.get(prop_name) {
                Some(value_prop_ty) => {
                    if !is_assignable_to(value_prop_ty, prop_ty) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Check methods
        for (method_name, method_ty) in &interface.methods {
            match value_members.get(method_name) {
                Some(value_method_ty) => {
                    if !is_assignable_to(value_method_ty, method_ty) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }

    /// Bind a variable type
    pub fn bind_variable(&mut self, name: String, ty: Type) {
        self.context.variables.insert(name, ty);
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// Exit current scope
    pub fn exit_scope(&mut self) {
        self.scope_depth = self.scope_depth.saturating_sub(1);
        self.narrowed_types.clear();
    }
}
