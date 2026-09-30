//! Dominator and Post-Dominator Tree Analysis
//!
//! Implements dominator tree construction for control dependency analysis.
//! Uses Cooper's algorithm (simpler than Lengauer-Tarjan, efficient for small graphs).

pub use crate::domain::semantic::pdg::dominator::{ControlDependencies, DominatorTree};
