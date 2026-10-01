//! Module graph for cross-file analysis
//!
//! This module provides:
//! - Import dependency graph building
//! - Circular import detection
//! - Topological sort for analysis order
//! - Module resolution across files

pub use crate::domain::modules::graph::ModuleGraph;
pub use crate::domain::modules::node::ModuleNode;
