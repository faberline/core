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

pub use crate::domain::rust_type_system::advanced::{
    apply_lifetime_elision, check_complex_trait_bounds, ArraySizeExpr, AssocTypeConstraint,
    BoundCheckResult, ComplexTraitBounds, ElisionResult, ElisionRule, ProjectionResolver,
    RustAdvancedInferencer, SizeOp,
};
