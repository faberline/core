//! Type narrowing based on control flow analysis
//!
//! This module handles type narrowing for:
//! - isinstance() checks
//! - None checks (is None, is not None)
//! - Truthiness checks
//! - Type guards

pub use crate::domain::narrowing::condition::NarrowingCondition;
pub use crate::domain::narrowing::narrower::TypeNarrower;
