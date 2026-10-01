//! Rust lifetime analysis
//!
//! This module provides lifetime analysis for Rust code, including:
//! - Lifetime constraint tracking
//! - Borrow checking (mutable vs immutable)
//! - Lifetime error reporting

pub use crate::domain::rust_type_system::lifetimes::{
    Borrow, BorrowId, BorrowState, LifetimeAnalyzer, LifetimeConstraint, LifetimeError,
    LifetimeErrorKind,
};
