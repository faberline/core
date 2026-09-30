//! Change-signature refactoring
//!
//! Modifies a function's parameter list (add, remove, reorder) and updates
//! every call site across the project to match the new signature.

pub use crate::domain::refactoring::signature::SignatureEngine;
