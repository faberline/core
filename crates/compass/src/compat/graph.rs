//! Cross-file import graph for project-wide dependency analysis
//!
//! Builds a directed graph where nodes are source files and edges are import
//! relationships. Supports circular dependency detection, unused file detection,
//! and incremental updates.

pub mod resolve;

pub use crate::domain::import_graph::graph::{GraphNode, ImportEdge, ImportGraph};
