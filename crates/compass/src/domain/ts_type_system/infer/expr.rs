use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::ts_type_system::infer::{is_falsy, is_truthy, TsTypeInferencer};
use crate::type_inference::{LiteralValue, Type};

impl<'a> TsTypeInferencer<'a> {
    /// Infer number literal (could be int or float, with literal type)
    pub(super) fn infer_number_literal(&self, node: &Node) -> Type {
        let text = self.node_text(node);
        if text.contains('.') || text.contains('e') || text.contains('E') {
            if let Ok(n) = text.parse::<f64>() {
                return Type::Literal(LiteralValue::Float(n));
            }
        } else if let Ok(n) = text.parse::<i64>() {
            return Type::Literal(LiteralValue::Int(n));
        }
        Type::Float // TypeScript uses 'number' for both
    }

    /// Infer string literal
    pub(super) fn infer_string_literal(&self, node: &Node) -> Type {
        let text = self.node_text(node);
        // Remove quotes
        let content = text
            .trim_start_matches(|c| c == '"' || c == '\'' || c == '`')
            .trim_end_matches(|c| c == '"' || c == '\'' || c == '`');

        if node.kind() == "template_string" {
            // Check if template has interpolations
            let mut cursor = node.walk();
            let has_interpolation = node
                .children(&mut cursor)
                .any(|c| c.kind() == "template_substitution");

            if has_interpolation {
                return Type::Str; // Complex template, return string
            }
        }

        Type::Literal(LiteralValue::Str(content.to_string()))
    }

    /// Infer identifier type
    pub(super) fn infer_identifier(&mut self, node: &Node) -> Type {
        let name = self.node_text(node);

        // Check narrowed types first (control flow analysis)
        if let Some(ty) = self.narrowed_types.get(name) {
            return ty.clone();
        }

        // Check variable bindings
        if let Some(ty) = self.context.variables.get(name) {
            return ty.clone();
        }

        // Check for type references
        if let Some(ty) = self.context.resolve_type(name) {
            return ty;
        }

        Type::Unknown
    }

    /// Infer binary expression type
    pub(super) fn infer_binary_expr(&mut self, node: &Node) -> Type {
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");
        let op = node.child_by_field_name("operator");

        // Extract op_text first before mutable borrows
        let op_text = op
            .map(|o| o.utf8_text(self.source.as_bytes()).unwrap_or(""))
            .unwrap_or("");

        let (left_ty, right_ty) = match (left, right) {
            (Some(l), Some(r)) => (self.infer_expr(&l), self.infer_expr(&r)),
            _ => return Type::Unknown,
        };

        match op_text {
            // Arithmetic
            "+" => match (&left_ty, &right_ty) {
                (Type::Str, _) | (_, Type::Str) => Type::Str,
                (Type::Literal(LiteralValue::Str(_)), _)
                | (_, Type::Literal(LiteralValue::Str(_))) => Type::Str,
                _ => Type::Float, // TypeScript number
            },
            "-" | "*" | "/" | "%" | "**" => Type::Float,

            // Comparison
            "==" | "===" | "!=" | "!==" | "<" | ">" | "<=" | ">=" => Type::Bool,

            // Logical
            "&&" => {
                // Returns right if left is truthy
                if is_falsy(&left_ty) {
                    left_ty
                } else {
                    right_ty
                }
            }
            "||" => {
                // Returns left if truthy, else right
                if is_truthy(&left_ty) {
                    left_ty
                } else {
                    Type::union(vec![left_ty, right_ty])
                }
            }
            "??" => {
                // Nullish coalescing: left if not null/undefined
                match &left_ty {
                    Type::None | Type::Optional(_) => right_ty,
                    _ => left_ty,
                }
            }

            // Bitwise
            "&" | "|" | "^" | "<<" | ">>" | ">>>" => Type::Int,

            // in / instanceof
            "in" | "instanceof" => Type::Bool,

            _ => Type::Unknown,
        }
    }

    /// Infer unary expression type
    pub(super) fn infer_unary_expr(&mut self, node: &Node) -> Type {
        let op = node.child_by_field_name("operator");
        let arg = node.child_by_field_name("argument");

        let op_text = op.map(|o| self.node_text(&o)).unwrap_or("");

        match op_text {
            "!" => Type::Bool,
            "+" | "-" => Type::Float,
            "~" => Type::Int,
            "typeof" => Type::Str,
            "void" => Type::None,
            "delete" => Type::Bool,
            _ => {
                if let Some(a) = arg {
                    self.infer_expr(&a)
                } else {
                    Type::Unknown
                }
            }
        }
    }

    /// Infer call expression with generic type inference
    pub fn infer_call_expr(&mut self, node: &Node) -> Type {
        let function = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return Type::Unknown,
        };

        let func_ty = self.infer_expr(&function);

        match func_ty {
            Type::Callable { params, ret } => {
                // Collect argument types
                let arg_types = self.collect_arguments(node);

                // Check for type variables that need inference
                let type_vars = ret.type_vars();
                if type_vars.is_empty() {
                    return (*ret).clone();
                }

                // Unify parameters with arguments to infer type variables
                let mut subs = HashMap::new();
                for (param, arg_ty) in params.iter().zip(arg_types.iter()) {
                    param.ty.unify(arg_ty, &mut subs);
                }

                // Apply substitutions to return type
                ret.substitute(&subs)
            }
            Type::ClassType { name, .. } => {
                // Constructor call returns instance
                Type::Instance {
                    name,
                    module: None,
                    type_args: vec![],
                }
            }
            _ => Type::Unknown,
        }
    }

    /// Collect argument types from a call expression
    fn collect_arguments(&mut self, node: &Node) -> Vec<Type> {
        let mut args = Vec::new();

        if let Some(args_node) = node.child_by_field_name("arguments") {
            let mut cursor = args_node.walk();
            for child in args_node.children(&mut cursor) {
                if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                    args.push(self.infer_expr(&child));
                }
            }
        }

        args
    }

    /// Infer member expression (property access)
    pub(super) fn infer_member_expr(&mut self, node: &Node) -> Type {
        let object = match node.child_by_field_name("object") {
            Some(o) => o,
            None => return Type::Unknown,
        };
        let property = match node.child_by_field_name("property") {
            Some(p) => p,
            None => return Type::Unknown,
        };

        let object_ty = self.infer_expr(&object);
        let prop_name = self.node_text(&property);

        self.get_property_type(&object_ty, prop_name)
    }

    /// Get property type from a type
    pub(super) fn get_property_type(&self, ty: &Type, prop: &str) -> Type {
        match ty {
            Type::Instance { name, .. } => {
                // Look up class
                if let Some(class) = self.context.classes.get(name) {
                    if let Some(prop_info) = class.properties.get(prop) {
                        return prop_info.ty.clone();
                    }
                    if let Some(method_ty) = class.methods.get(prop) {
                        return method_ty.clone();
                    }
                }
                // Look up interface
                if let Some(iface) = self.context.interfaces.get(name) {
                    if let Some(prop_ty) = iface.properties.get(prop) {
                        return prop_ty.clone();
                    }
                    if let Some(prop_ty) = iface.optional_properties.get(prop) {
                        return Type::optional(prop_ty.clone());
                    }
                    if let Some(method_ty) = iface.methods.get(prop) {
                        return method_ty.clone();
                    }
                }
                Type::Unknown
            }
            Type::Protocol { members, .. } => {
                for (name, member_ty) in members {
                    if name == prop {
                        return member_ty.clone();
                    }
                }
                Type::Unknown
            }
            Type::List(_) => {
                // Array methods
                match prop {
                    "length" => Type::Int,
                    "push" | "pop" | "shift" | "unshift" | "splice" | "slice" | "concat"
                    | "join" | "map" | "filter" | "reduce" | "forEach" => {
                        Type::Any // Simplified
                    }
                    _ => Type::Unknown,
                }
            }
            Type::Str => match prop {
                "length" => Type::Int,
                "charAt" | "substring" | "slice" | "trim" | "toLowerCase" | "toUpperCase"
                | "split" | "replace" => Type::Any,
                _ => Type::Unknown,
            },
            Type::Union(members) => {
                // Get property from all union members
                let prop_types: Vec<Type> = members
                    .iter()
                    .map(|m| self.get_property_type(m, prop))
                    .filter(|t| !matches!(t, Type::Unknown))
                    .collect();

                if prop_types.is_empty() {
                    Type::Unknown
                } else if prop_types.len() == 1 {
                    prop_types.into_iter().next().unwrap()
                } else {
                    Type::union(prop_types)
                }
            }
            Type::Optional(inner) => Type::optional(self.get_property_type(inner, prop)),
            _ => Type::Unknown,
        }
    }
}
