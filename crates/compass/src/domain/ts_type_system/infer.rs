//! TypeScript type inference engine
//!
//! Implements TypeScript-specific type inference including:
//! - Generic type inference with constraints
//! - Union and intersection type handling
//! - Structural subtyping for interfaces
//! - Control flow type narrowing
//! - Literal types and template literals

use std::collections::HashMap;

use tree_sitter::Node;

#[allow(unused_imports)]
use crate::domain::ts_type_system::types::{is_assignable_to, TsInterface, TsTypeContext};
use crate::type_inference::{LiteralValue, Type, TypeVarId, Variance};

mod compound_expr;
mod expr;
mod narrowing;
mod type_annotation;
mod type_operators;

/// TypeScript type inferencer
pub struct TsTypeInferencer<'a> {
    /// Source code
    source: &'a str,
    /// Type context (interfaces, classes, aliases)
    context: TsTypeContext,
    /// Type variable substitutions
    #[allow(dead_code)]
    substitutions: HashMap<TypeVarId, Type>,
    /// Control flow type narrowing
    narrowed_types: HashMap<String, Type>,
    /// Counter for fresh type variables
    next_type_var_id: usize,
    /// Current scope depth
    scope_depth: usize,
}

impl<'a> TsTypeInferencer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            context: TsTypeContext::new(),
            substitutions: HashMap::new(),
            narrowed_types: HashMap::new(),
            next_type_var_id: 0,
            scope_depth: 0,
        }
    }

    /// Get the type context
    pub fn context(&self) -> &TsTypeContext {
        &self.context
    }

    /// Get mutable access to the type context
    pub fn context_mut(&mut self) -> &mut TsTypeContext {
        &mut self.context
    }

    /// Get node text
    fn node_text(&self, node: &Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Generate a fresh type variable
    pub fn fresh_type_var(&mut self, name: &str) -> Type {
        let id = TypeVarId(self.next_type_var_id);
        self.next_type_var_id += 1;
        Type::TypeVar {
            id,
            name: name.to_string(),
            bound: None,
            constraints: vec![],
            variance: Variance::Invariant,
        }
    }

    /// Generate a fresh type variable with constraint
    pub fn fresh_type_var_bounded(&mut self, name: &str, bound: Type) -> Type {
        let id = TypeVarId(self.next_type_var_id);
        self.next_type_var_id += 1;
        Type::TypeVar {
            id,
            name: name.to_string(),
            bound: Some(Box::new(bound)),
            constraints: vec![],
            variance: Variance::Invariant,
        }
    }

    /// Infer the type of an expression
    pub fn infer_expr(&mut self, node: &Node) -> Type {
        if node.is_error() || node.is_missing() {
            return Type::Error;
        }

        match node.kind() {
            // Literals
            "number" => self.infer_number_literal(node),
            "string" | "template_string" => self.infer_string_literal(node),
            "true" => Type::Literal(LiteralValue::Bool(true)),
            "false" => Type::Literal(LiteralValue::Bool(false)),
            "null" => Type::None,
            "undefined" => Type::None,

            // Identifier lookup
            "identifier" => self.infer_identifier(node),

            // Binary expressions
            "binary_expression" => self.infer_binary_expr(node),

            // Unary expressions
            "unary_expression" => self.infer_unary_expr(node),

            // Call expression
            "call_expression" => self.infer_call_expr(node),

            // Member expression (property access)
            "member_expression" => self.infer_member_expr(node),

            // Object literal
            "object" => self.infer_object_literal(node),

            // Array literal
            "array" => self.infer_array_literal(node),

            // Arrow function
            "arrow_function" => self.infer_arrow_function(node),

            // Function expression
            "function_expression" | "function" => self.infer_function_expr(node),

            // Conditional (ternary)
            "ternary_expression" => self.infer_ternary_expr(node),

            // As expression (type assertion)
            "as_expression" => self.infer_as_expr(node),

            // Type assertion (<Type>expr)
            "type_assertion" => self.infer_type_assertion(node),

            // Parenthesized expression
            "parenthesized_expression" => {
                if let Some(inner) = node.child(1) {
                    self.infer_expr(&inner)
                } else {
                    Type::Unknown
                }
            }

            // Await expression
            "await_expression" => {
                if let Some(arg) = node.child(1) {
                    // Unwrap Promise<T> -> T
                    let inner_ty = self.infer_expr(&arg);
                    self.unwrap_promise(inner_ty)
                } else {
                    Type::Unknown
                }
            }

            // New expression
            "new_expression" => self.infer_new_expr(node),

            _ => Type::Unknown,
        }
    }
}

/// Check if a type is definitely falsy
fn is_falsy(ty: &Type) -> bool {
    match ty {
        Type::None | Type::Never => true,
        Type::Literal(LiteralValue::Bool(false)) => true,
        Type::Literal(LiteralValue::Int(0)) => true,
        Type::Literal(LiteralValue::Str(s)) if s.is_empty() => true,
        _ => false,
    }
}

/// Check if a type is definitely truthy
fn is_truthy(ty: &Type) -> bool {
    match ty {
        Type::Literal(LiteralValue::Bool(true)) => true,
        Type::Literal(LiteralValue::Int(n)) if *n != 0 => true,
        Type::Literal(LiteralValue::Str(s)) if !s.is_empty() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
