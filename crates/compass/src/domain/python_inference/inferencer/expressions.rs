use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::ty::Type;

impl<'a> TypeInferencer<'a> {
    /// Infer the type of an expression
    ///
    /// Returns `Type::Error` for error nodes (from parser recovery) to prevent
    /// cascading errors in downstream type checking.
    pub fn infer_expr(&mut self, node: &Node) -> Type {
        // Error recovery: return Error type for ERROR nodes from tree-sitter
        // This prevents cascading type errors when parsing fails
        if node.is_error() || node.is_missing() {
            return Type::Error;
        }

        match node.kind() {
            // Explicit ERROR node kind (fallback check)
            "ERROR" => Type::Error,

            // Literals
            "integer" => Type::Int,
            "float" => Type::Float,
            "string" => Type::Str,
            "true" | "false" => Type::Bool,
            "none" => Type::None,
            "ellipsis" => Type::Any, // ... is typically used as placeholder

            // Identifier lookup
            "identifier" => {
                let name = self.node_text(node);
                // Check type overrides first (from narrowing)
                if let Some(ref overrides) = self.type_overrides {
                    if let Some(ty) = overrides.get(name) {
                        return ty.clone();
                    }
                }
                // Use Error type for unresolved identifiers in error regions
                // This helps prevent cascading errors
                self.env.lookup(name).cloned().unwrap_or(Type::Unknown)
            }

            // Binary operators
            "binary_operator" => self.infer_binary_op(node),

            // Unary operators
            "unary_operator" => self.infer_unary_op(node),

            // Comparison operators
            "comparison_operator" => Type::Bool,
            "boolean_operator" => Type::Bool,
            "not_operator" => Type::Bool,

            // Container literals
            "list" => self.infer_list_literal(node),
            "dictionary" => self.infer_dict_literal(node),
            "set" => self.infer_set_literal(node),
            "tuple" => self.infer_tuple_literal(node),

            // List comprehension
            "list_comprehension" => self.infer_list_comprehension(node),

            // Call expression
            "call" => self.infer_call(node),

            // Attribute access
            "attribute" => self.infer_attribute(node),

            // Subscript
            "subscript" => self.infer_subscript(node),

            // Conditional expression (ternary)
            "conditional_expression" => self.infer_conditional(node),

            // Lambda
            "lambda" => self.infer_lambda(node),

            // Await expression
            "await" => {
                if let Some(arg) = node.child(1) {
                    // Unwrap Awaitable[T] -> T
                    let inner = self.infer_expr(&arg);
                    // For now, just return the inner type
                    inner
                } else {
                    Type::Unknown
                }
            }

            // Parenthesized expression
            "parenthesized_expression" => {
                if let Some(inner) = node.child(1) {
                    self.infer_expr(&inner)
                } else {
                    Type::Unknown
                }
            }

            _ => Type::Unknown,
        }
    }

    /// Infer binary operator result type
    fn infer_binary_op(&mut self, node: &Node) -> Type {
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");
        let op = node.child_by_field_name("operator");

        let (left_ty, right_ty) = match (left, right) {
            (Some(l), Some(r)) => (self.infer_expr(&l), self.infer_expr(&r)),
            _ => return Type::Unknown,
        };

        let op_text = op.map(|o| self.node_text(&o)).unwrap_or("");

        match op_text {
            // Arithmetic operators
            "+" => match (&left_ty, &right_ty) {
                (Type::Str, Type::Str) => Type::Str,
                (Type::List(a), Type::List(b)) if a == b => Type::list((**a).clone()),
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, _) | (_, Type::Float) => Type::Float,
                _ => Type::Unknown,
            },
            "-" | "*" => match (&left_ty, &right_ty) {
                (Type::Str, Type::Int) if op_text == "*" => Type::Str, // "a" * 3
                (Type::Int, Type::Str) if op_text == "*" => Type::Str, // 3 * "a"
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, _) | (_, Type::Float) => Type::Float,
                _ => Type::Unknown,
            },
            "/" => Type::Float, // Python 3 true division
            "//" => Type::Int,  // Floor division
            "%" => match (&left_ty, &right_ty) {
                (Type::Str, _) => Type::Str, // String formatting
                (Type::Int, Type::Int) => Type::Int,
                _ => Type::Unknown,
            },
            "**" => match (&left_ty, &right_ty) {
                (Type::Int, Type::Int) => Type::Int,
                _ => Type::Float,
            },

            // Bitwise operators
            "&" | "|" | "^" | "<<" | ">>" => Type::Int,

            // Membership/identity
            "in" | "not in" | "is" | "is not" => Type::Bool,

            _ => Type::Unknown,
        }
    }

    /// Infer unary operator result type
    fn infer_unary_op(&mut self, node: &Node) -> Type {
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        let op = children.first().map(|n| self.node_text(n)).unwrap_or("");
        let operand = children.get(1);

        match op {
            "-" | "+" => {
                if let Some(arg) = operand {
                    let arg_ty = self.infer_expr(arg);
                    match arg_ty {
                        Type::Int => Type::Int,
                        Type::Float => Type::Float,
                        _ => Type::Unknown,
                    }
                } else {
                    Type::Unknown
                }
            }
            "~" => Type::Int, // Bitwise NOT
            "not" => Type::Bool,
            _ => Type::Unknown,
        }
    }

    /// Infer list literal type
    fn infer_list_literal(&mut self, node: &Node) -> Type {
        let mut cursor = node.walk();
        let mut element_types = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() != "[" && child.kind() != "]" && child.kind() != "," {
                element_types.push(self.infer_expr(&child));
            }
        }

        if element_types.is_empty() {
            Type::list(Type::Unknown)
        } else {
            // Use the first element's type (simplified)
            // A full implementation would compute LUB (least upper bound)
            Type::list(element_types[0].clone())
        }
    }

    /// Infer dict literal type
    fn infer_dict_literal(&mut self, node: &Node) -> Type {
        let mut cursor = node.walk();
        let mut key_type = Type::Unknown;
        let mut value_type = Type::Unknown;

        for child in node.children(&mut cursor) {
            if child.kind() == "pair" {
                if let Some(key) = child.child_by_field_name("key") {
                    key_type = self.infer_expr(&key);
                }
                if let Some(value) = child.child_by_field_name("value") {
                    value_type = self.infer_expr(&value);
                }
                break; // Just use first pair for type
            }
        }

        Type::dict(key_type, value_type)
    }

    /// Infer set literal type
    fn infer_set_literal(&mut self, node: &Node) -> Type {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() != "{" && child.kind() != "}" && child.kind() != "," {
                let elem_ty = self.infer_expr(&child);
                return Type::Set(Box::new(elem_ty));
            }
        }

        Type::Set(Box::new(Type::Unknown))
    }

    /// Infer tuple literal type
    fn infer_tuple_literal(&mut self, node: &Node) -> Type {
        let mut cursor = node.walk();
        let mut element_types = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                element_types.push(self.infer_expr(&child));
            }
        }

        Type::Tuple(element_types)
    }

    /// Infer list comprehension type
    fn infer_list_comprehension(&mut self, node: &Node) -> Type {
        // [expr for x in iter] -> list[type of expr]
        if let Some(body) = node.child(1) {
            // This is simplified - should handle the iteration binding
            let elem_ty = self.infer_expr(&body);
            Type::list(elem_ty)
        } else {
            Type::list(Type::Unknown)
        }
    }
}
