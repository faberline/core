//! Advanced TypeScript type inference (R1)
//!
//! Extends the existing TypeScript type system (`ts_types`, `ts_infer`) with
//! handlers for the advanced constructs most common in modern TypeScript:
//!
//! - **Generics** — constraint checking and type-argument substitution
//! - **Mapped types** — `{ [K in keyof T]: U }` evaluation
//! - **Conditional types** — `T extends U ? X : Y` evaluation
//! - **Template literal types** — `` `${string}-${number}` `` matching
//!
//! The `AdvancedTsTypeInferencer` is used by `hover` and `type-at` handlers
//! to produce accurate type information when the cursor sits on a complex
//! expression whose type cannot be resolved by the base inferencer alone.

pub use crate::domain::ts_type_system::advanced::{
    AdvancedTsTypeInferencer, ConstraintResult, TsGenericApplication,
};
