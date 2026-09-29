//! Rust-specific type system extensions
//!
//! This module provides Rust-specific type constructs for the Lens type system,
//! including traits, lifetimes, and Rust-specific type inference.

pub use crate::domain::rust_type_system::types::{
    AssociatedType, ClosureKind, EnumDef, EnumVariant, ImplBlock, ImplMethod, Lifetime, LifetimeId,
    RustParam, RustType, RustTypeParam, SelfParam, StructDef, StructField, StructFields,
    TraitBound, TraitDef, TraitId, TraitMethod, TraitRef, Visibility, WherePredicate,
};
