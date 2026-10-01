//! Go code checker (R5)
//!
//! Implements 10 built-in lint rules (GO000–GO010) plus module-based import
//! graph analysis for interface-satisfaction checking.

pub use crate::domain::lint::go::GoChecker;
