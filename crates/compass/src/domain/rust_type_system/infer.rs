//! Rust type inference engine
//!
//! This module provides type inference for Rust code, including:
//! - Generic type resolution
//! - Trait bound checking
//! - Method resolution
//! - Pattern matching type inference

use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::rust_type_system::types::{ImplBlock, Lifetime, LifetimeId, RustType, TraitDef};
use crate::type_inference::TypeVarId;

mod bound_syntax;
mod compound_expr;
mod context;
mod elision;
mod expr;
mod type_syntax;
mod unify;

pub use context::RustTypeContext;

// ============================================================================
// Rust Type Inferencer
// ============================================================================

/// Rust type inference engine
pub struct RustTypeInferencer {
    /// Type context
    context: RustTypeContext,
    /// Trait resolver for method resolution
    trait_resolver: crate::domain::rust_type_system::traits::TraitResolver,
    /// Type substitutions (for unification)
    substitutions: HashMap<TypeVarId, RustType>,
    /// Lifetime substitutions (reserved for future lifetime analysis)
    #[allow(dead_code)]
    lifetime_substitutions: HashMap<LifetimeId, Lifetime>,
    /// Inference errors
    errors: Vec<RustTypeError>,
}

/// Type inference error
#[derive(Debug, Clone)]
pub struct RustTypeError {
    /// Error message
    pub message: String,
    /// Location in source
    pub span: Option<(usize, usize)>,
    /// Error kind
    pub kind: RustTypeErrorKind,
}

/// Kind of type error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustTypeErrorKind {
    /// Type mismatch
    TypeMismatch,
    /// Unbound type variable
    UnboundTypeVar,
    /// Trait not implemented
    TraitNotImplemented,
    /// Lifetime error
    LifetimeError,
    /// Borrow checker error
    BorrowError,
    /// Ambiguous type
    AmbiguousType,
}

impl RustTypeInferencer {
    /// Create a new type inferencer
    pub fn new() -> Self {
        Self {
            context: RustTypeContext::new(),
            trait_resolver: crate::domain::rust_type_system::traits::TraitResolver::new(),
            substitutions: HashMap::new(),
            lifetime_substitutions: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Create with existing context
    pub fn with_context(context: RustTypeContext) -> Self {
        // Build trait resolver from context's trait impls and defs
        let mut trait_resolver = crate::domain::rust_type_system::traits::TraitResolver::new();
        for impl_block in &context.trait_impls {
            trait_resolver.register_impl((**impl_block).clone());
        }
        for (_, trait_def) in &context.trait_defs {
            trait_resolver.register_trait((**trait_def).clone());
        }

        Self {
            context,
            trait_resolver,
            substitutions: HashMap::new(),
            lifetime_substitutions: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Get a reference to the trait resolver
    pub fn trait_resolver(&self) -> &crate::domain::rust_type_system::traits::TraitResolver {
        &self.trait_resolver
    }

    /// Register an impl block for method resolution
    pub fn register_impl(&mut self, impl_block: ImplBlock) {
        self.trait_resolver.register_impl(impl_block);
    }

    /// Register a trait definition
    pub fn register_trait(&mut self, trait_def: TraitDef) {
        self.trait_resolver.register_trait(trait_def);
    }

    /// Get inference errors
    pub fn errors(&self) -> &[RustTypeError] {
        &self.errors
    }

    /// Clear errors
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Infer the type of a Rust expression from AST node
    pub fn infer_expr(&mut self, node: &Node, source: &str) -> RustType {
        let kind = node.kind();

        match kind {
            // Literals
            "integer_literal" => self.infer_integer_literal(node, source),
            "float_literal" => RustType::F64,
            "string_literal" | "raw_string_literal" => RustType::Reference {
                lifetime: Some(Lifetime::Static),
                mutable: false,
                inner: Box::new(RustType::Str),
            },
            "char_literal" => RustType::Char,
            "boolean_literal" => RustType::Bool,

            // Identifiers
            "identifier" => self.infer_identifier(node, source),

            // Compound expressions
            "call_expression" => self.infer_call_expr(node, source),
            "field_expression" => self.infer_field_expr(node, source),
            "index_expression" => self.infer_index_expr(node, source),
            "reference_expression" => self.infer_reference_expr(node, source),
            "dereference_expression" => self.infer_deref_expr(node, source),
            "binary_expression" => self.infer_binary_expr(node, source),
            "unary_expression" => self.infer_unary_expr(node, source),
            "if_expression" => self.infer_if_expr(node, source),
            "match_expression" => self.infer_match_expr(node, source),
            "block" => self.infer_block(node, source),
            "tuple_expression" => self.infer_tuple_expr(node, source),
            "array_expression" => self.infer_array_expr(node, source),
            "struct_expression" => self.infer_struct_expr(node, source),
            "closure_expression" => self.infer_closure_expr(node, source),

            // Unit
            "unit_expression" => RustType::Unit,

            // Unknown
            _ => {
                self.errors.push(RustTypeError {
                    message: format!("Unknown expression kind: {}", kind),
                    span: Some((node.start_byte(), node.end_byte())),
                    kind: RustTypeErrorKind::AmbiguousType,
                });
                RustType::Infer
            }
        }
    }
}

impl Default for RustTypeInferencer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
