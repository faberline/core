//! Move-definition refactoring
//!
//! Moves a function, class, or type definition from one file to another,
//! then rewrites imports across the entire project so that existing
//! consumers still resolve correctly.

pub use crate::domain::refactoring::move_def::MoveDefEngine;
