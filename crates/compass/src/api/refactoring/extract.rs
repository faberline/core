//! Extract function / method / variable refactoring
//!
//! - **Extract Function**: analyses data flow and lifts a selection into a
//!   standalone function.
//! - **Extract Method**: same but adds `self` and class-level indentation.
//! - **Extract Variable**: replaces an expression with a named variable.

pub use crate::domain::refactoring::extract::ExtractEngine;
