use tree_sitter::Node;

use crate::domain::rust_type_system::infer::RustTypeInferencer;
use crate::domain::rust_type_system::types::{ClosureKind, RustType};

impl RustTypeInferencer {
    /// Infer binary expression type
    pub(super) fn infer_binary_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let (Some(left), Some(op), Some(right)) = (
            node.child_by_field_name("left"),
            node.child_by_field_name("operator"),
            node.child_by_field_name("right"),
        ) {
            let left_type = self.infer_expr(&left, source);
            let _right_type = self.infer_expr(&right, source);
            let operator = &source[op.start_byte()..op.end_byte()];

            // Comparison operators return bool
            if matches!(
                operator,
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
            ) {
                return RustType::Bool;
            }

            // Arithmetic operators return the same type
            left_type
        } else {
            RustType::Infer
        }
    }

    /// Infer unary expression type
    pub(super) fn infer_unary_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let (Some(op), Some(value)) = (
            node.child_by_field_name("operator"),
            node.child_by_field_name("value"),
        ) {
            let inner_type = self.infer_expr(&value, source);
            let operator = &source[op.start_byte()..op.end_byte()];

            match operator {
                "!" => RustType::Bool,
                "-" => inner_type,
                "*" => match inner_type {
                    RustType::Reference { inner, .. } => *inner,
                    RustType::RawPointer { inner, .. } => *inner,
                    _ => RustType::Infer,
                },
                _ => inner_type,
            }
        } else {
            RustType::Infer
        }
    }

    /// Infer if expression type
    pub(super) fn infer_if_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let Some(consequence) = node.child_by_field_name("consequence") {
            self.infer_block(&consequence, source)
        } else {
            RustType::Unit
        }
    }

    /// Infer match expression type
    pub(super) fn infer_match_expr(&mut self, node: &Node, source: &str) -> RustType {
        // Get type from first arm
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "match_arm" {
                if let Some(value) = child.child_by_field_name("value") {
                    return self.infer_expr(&value, source);
                }
            }
        }
        RustType::Never
    }

    /// Infer block type (type of last expression)
    pub(super) fn infer_block(&mut self, node: &Node, source: &str) -> RustType {
        let mut last_type = RustType::Unit;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "expression_statement" => {
                    if let Some(expr) = child.child(0) {
                        let _ = self.infer_expr(&expr, source);
                    }
                    last_type = RustType::Unit; // Statement, not expression
                }
                "let_declaration" => {
                    self.process_let_declaration(&child, source);
                    last_type = RustType::Unit;
                }
                _ if !matches!(child.kind(), "{" | "}") => {
                    // Last expression without semicolon
                    last_type = self.infer_expr(&child, source);
                }
                _ => {}
            }
        }

        last_type
    }

    /// Process let declaration and add binding
    fn process_let_declaration(&mut self, node: &Node, source: &str) {
        if let (Some(pattern), Some(value)) = (
            node.child_by_field_name("pattern"),
            node.child_by_field_name("value"),
        ) {
            let value_type = self.infer_expr(&value, source);
            let pattern_name = &source[pattern.start_byte()..pattern.end_byte()];

            // Check for explicit type annotation
            if let Some(type_node) = node.child_by_field_name("type") {
                let annotated_type = self.parse_type(&type_node, source);
                self.context
                    .bind_type(pattern_name.to_string(), annotated_type);
            } else {
                self.context.bind_type(pattern_name.to_string(), value_type);
            }
        }
    }

    /// Infer tuple expression type
    pub(super) fn infer_tuple_expr(&mut self, node: &Node, source: &str) -> RustType {
        let mut elements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if !matches!(child.kind(), "(" | ")" | ",") {
                elements.push(self.infer_expr(&child, source));
            }
        }

        RustType::Tuple(elements)
    }

    /// Infer array expression type
    pub(super) fn infer_array_expr(&mut self, node: &Node, source: &str) -> RustType {
        let mut elements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if !matches!(child.kind(), "[" | "]" | "," | ";") {
                elements.push(self.infer_expr(&child, source));
            }
        }

        if let Some(first) = elements.first() {
            RustType::Array {
                element: Box::new(first.clone()),
                size: elements.len(),
            }
        } else {
            RustType::Array {
                element: Box::new(RustType::Infer),
                size: 0,
            }
        }
    }

    /// Infer struct expression type
    pub(super) fn infer_struct_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = &source[name_node.start_byte()..name_node.end_byte()];
            RustType::Named {
                name: name.to_string(),
                module: None,
                type_args: vec![],
                lifetime_args: vec![],
            }
        } else {
            RustType::Infer
        }
    }

    /// Infer closure expression type
    pub(super) fn infer_closure_expr(&mut self, node: &Node, source: &str) -> RustType {
        let mut params = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "parameter" {
                    params.push(RustType::Infer);
                }
            }
        }

        let return_type = if let Some(body) = node.child_by_field_name("body") {
            self.infer_expr(&body, source)
        } else {
            RustType::Infer
        };

        RustType::Closure {
            kind: ClosureKind::Fn, // Default, will be refined
            params,
            return_type: Box::new(return_type),
        }
    }
}
