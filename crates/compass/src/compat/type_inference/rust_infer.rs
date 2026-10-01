//! Rust type inference engine
//!
//! This module provides type inference for Rust code, including:
//! - Generic type resolution
//! - Trait bound checking
//! - Method resolution
//! - Pattern matching type inference

// Re-export from submodules
pub use super::rust_lifetimes::{
    Borrow, BorrowId, BorrowState, LifetimeAnalyzer, LifetimeConstraint, LifetimeError,
    LifetimeErrorKind,
};
pub use super::rust_symbols::{
    RustConstant, RustFunction, RustSymbolCollector, RustSymbols, RustTypeAlias,
};
pub use super::rust_traits::{MethodResolution, TraitResolver};

pub use crate::domain::rust_type_system::infer::{
    RustTypeContext, RustTypeError, RustTypeErrorKind, RustTypeInferencer,
};
