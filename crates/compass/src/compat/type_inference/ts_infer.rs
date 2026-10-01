//! TypeScript type inference engine
//!
//! Implements TypeScript-specific type inference including:
//! - Generic type inference with constraints
//! - Union and intersection type handling
//! - Structural subtyping for interfaces
//! - Control flow type narrowing
//! - Literal types and template literals

pub use crate::domain::ts_type_system::infer::TsTypeInferencer;
