//! Advanced Rust type inference (R2)
//!
//! Extends the existing Rust type system (`rust_types`, `rust_infer`) with
//! handlers for the complex Rust-specific constructs that the base inferencer
//! does not cover:
//!
//! - **Array size expressions** — const-generic expressions `[T; N*2]`
//! - **Complex trait bounds** — multi-bound where clauses with associated-type
//!   equality constraints (`T: Iterator<Item = u32> + Send + 'static`)
//! - **Associated type projections** — `<T as Trait>::Assoc` resolution
//! - **Lifetime elision** — all three standard elision rules for `fn` signatures
//!   and `impl` blocks

use std::collections::HashMap;

use crate::domain::rust_type_system::infer::RustTypeContext;
use crate::domain::rust_type_system::types::RustType;

mod elision;
mod projection;
mod trait_bounds;

pub use elision::{apply_lifetime_elision, ElisionResult, ElisionRule};
pub use projection::ProjectionResolver;
pub use trait_bounds::{
    check_complex_trait_bounds, AssocTypeConstraint, BoundCheckResult, ComplexTraitBounds,
};

// ============================================================================
// R2a: Array size expressions
// ============================================================================

/// A constant expression that may appear as an array size.
///
/// Rust supports full const-generic expressions: `[T; N]`, `[T; N + M]`,
/// `[T; size_of::<u64>()]`, etc.  This type models the subset that Lens
/// needs to evaluate for `hover` / `type-at` purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum ArraySizeExpr {
    /// A literal integer size
    Literal(usize),
    /// A named const-generic parameter
    ConstParam(String),
    /// Binary arithmetic expression
    BinOp {
        op: SizeOp,
        lhs: Box<ArraySizeExpr>,
        rhs: Box<ArraySizeExpr>,
    },
}

/// Binary operator for array size expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl ArraySizeExpr {
    /// Attempt to evaluate the expression given a mapping of const-param names
    /// to their concrete `usize` values.
    ///
    /// Returns `None` when a const parameter is not in `env` or on arithmetic
    /// overflow / division-by-zero.
    pub fn evaluate(&self, env: &HashMap<String, usize>) -> Option<usize> {
        match self {
            ArraySizeExpr::Literal(n) => Some(*n),
            ArraySizeExpr::ConstParam(name) => env.get(name).copied(),
            ArraySizeExpr::BinOp { op, lhs, rhs } => {
                let l = lhs.evaluate(env)?;
                let r = rhs.evaluate(env)?;
                match op {
                    SizeOp::Add => l.checked_add(r),
                    SizeOp::Sub => l.checked_sub(r),
                    SizeOp::Mul => l.checked_mul(r),
                    SizeOp::Div => {
                        if r == 0 {
                            None
                        } else {
                            Some(l / r)
                        }
                    }
                }
            }
        }
    }

    /// Resolve the `ArraySizeExpr` to a concrete `RustType::Array` variant
    /// given a const-param environment.
    pub fn into_array_type(&self, element: RustType, env: &HashMap<String, usize>) -> RustType {
        let size = self.evaluate(env).unwrap_or(0);
        RustType::Array {
            element: Box::new(element),
            size,
        }
    }
}

/// Structural type equality check (ignores lifetime arguments for simplicity).
fn types_match(a: &RustType, b: &RustType) -> bool {
    match (a, b) {
        (
            RustType::Named {
                name: n1,
                type_args: ta1,
                ..
            },
            RustType::Named {
                name: n2,
                type_args: ta2,
                ..
            },
        ) => {
            n1 == n2
                && ta1.len() == ta2.len()
                && ta1.iter().zip(ta2.iter()).all(|(x, y)| types_match(x, y))
        }
        (RustType::TypeParam { name: n1, .. }, RustType::TypeParam { name: n2, .. }) => n1 == n2,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

// ============================================================================
// RustAdvancedInferencer — convenience wrapper
// ============================================================================

/// High-level helper that bundles projection resolution and bound checking.
pub struct RustAdvancedInferencer<'ctx> {
    ctx: &'ctx RustTypeContext,
    projection_resolver: ProjectionResolver<'ctx>,
    pub lifetime_counter: usize,
}

impl<'ctx> RustAdvancedInferencer<'ctx> {
    /// Create a new inferencer from an existing `RustTypeContext`.
    pub fn new(ctx: &'ctx RustTypeContext) -> Self {
        Self {
            ctx,
            projection_resolver: ProjectionResolver::new(ctx),
            lifetime_counter: 0,
        }
    }

    /// Resolve an associated type projection.
    pub fn resolve_projection(
        &self,
        concrete_type: &RustType,
        trait_name: &str,
        assoc_name: &str,
    ) -> Option<RustType> {
        self.projection_resolver
            .resolve_projection(concrete_type, trait_name, assoc_name)
    }

    /// Check complex trait bounds for a concrete type.
    pub fn check_bounds(
        &self,
        concrete_type: &RustType,
        spec: &ComplexTraitBounds,
    ) -> BoundCheckResult {
        check_complex_trait_bounds(self.ctx, concrete_type, spec)
    }

    /// Evaluate an array size expression.
    pub fn evaluate_array_size(
        &self,
        expr: &ArraySizeExpr,
        env: &HashMap<String, usize>,
    ) -> Option<usize> {
        expr.evaluate(env)
    }

    /// Apply lifetime elision to a function signature.
    pub fn apply_elision(
        &mut self,
        has_self_ref: bool,
        input_elided_count: usize,
        has_elided_output: bool,
    ) -> ElisionResult {
        apply_lifetime_elision(
            has_self_ref,
            input_elided_count,
            has_elided_output,
            &mut self.lifetime_counter,
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
