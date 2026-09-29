//! Program Dependence Graph (PDG) for Python
//!
//! Combines control dependencies and data dependencies into a unified
//! graph for program analysis. Supports:
//! - Forward and backward slicing
//! - Impact analysis
//! - Taint tracking
//! - Dead code detection

pub(crate) mod cfg;
pub(crate) mod data_flow;
mod dead_code;
pub(crate) mod dominator;
mod impact;
mod impact_tree;
mod json;
mod semantic_taint;
mod slice;
mod stats;
mod taint;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::syntax::parsed_file::ParsedFile;
use crate::type_inference::Span;

use cfg::{BlockId, CfgBuilder, ControlFlowGraph, StatementKind};
use data_flow::DataDependencies;
pub use dead_code::DeadCodeAnalysis;
use dominator::ControlDependencies;
pub use impact::ImpactAnalysis;
pub use impact_tree::{DependencyReason, ImpactAnalysisTree, ImpactTreeNode};
pub use json::{PdgEdgeJson, PdgJson, PdgNodeJson, PdgStatsJson};
pub use semantic_taint::{
    detect_taint_sinks, detect_taint_sources, DetectedSink, DetectedSource, SemanticTaintAnalysis,
    TaintSinkKind, TaintSourceKind,
};
pub use slice::{ProgramSlice, SliceDirection};
pub use stats::PdgStats;
pub use taint::{TaintAnalysis, TaintPath};

/// A node in the PDG (represents a statement)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PdgNode {
    /// Unique identifier
    pub id: u32,
    /// Line number (0-indexed)
    pub line: usize,
    /// Span in source
    pub span: Span,
    /// Statement text
    pub text: String,
    /// CFG block ID
    pub block: BlockId,
    /// Statement kind
    pub kind: StatementKind,
}

/// Type of PDG edge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdgEdgeKind {
    /// Control dependency
    Control,
    /// Data dependency (with variable name)
    Data { variable: String },
}

/// An edge in the PDG
#[derive(Debug, Clone)]
pub struct PdgEdge {
    /// Source node
    pub from: u32,
    /// Target node
    pub to: u32,
    /// Edge kind
    pub kind: PdgEdgeKind,
}

/// Program Dependence Graph
#[derive(Debug, Clone)]
pub struct ProgramDependenceGraph {
    /// All nodes (statement ID -> node)
    pub nodes: HashMap<u32, PdgNode>,
    /// Forward edges (from -> to)
    pub edges: HashMap<u32, Vec<PdgEdge>>,
    /// Backward edges (to -> from)
    pub reverse_edges: HashMap<u32, Vec<PdgEdge>>,
    /// Line to node mapping (supports multiple nodes per line)
    pub line_to_nodes: HashMap<usize, Vec<u32>>,
    /// Function name (if for a specific function)
    pub function_name: Option<String>,
    /// Source file path
    pub file_path: Option<PathBuf>,
    /// Control flow graph
    pub cfg: ControlFlowGraph,
    /// Data dependencies
    pub data_deps: DataDependencies,
    /// Control dependencies
    pub control_deps: ControlDependencies,
}

impl ProgramDependenceGraph {
    /// Build PDG from source code
    pub fn build(source: &str, file: &ParsedFile) -> Self {
        let cfg = CfgBuilder::new(source).build(file);
        Self::from_cfg(cfg, file)
    }

    /// Build PDG from an existing CFG
    pub fn from_cfg(cfg: ControlFlowGraph, file: &ParsedFile) -> Self {
        // Compute dependencies
        let data_deps = DataDependencies::compute(&cfg, file);
        let control_deps = ControlDependencies::compute(&cfg);

        // Create nodes from CFG statements
        let mut nodes = HashMap::new();
        let mut line_to_nodes: HashMap<usize, Vec<u32>> = HashMap::new();
        let mut next_id = 0u32;

        for block in cfg.blocks.values() {
            for stmt in &block.statements {
                let id = next_id;
                next_id += 1;

                nodes.insert(
                    id,
                    PdgNode {
                        id,
                        line: stmt.line,
                        span: stmt.span,
                        text: stmt.text.clone(),
                        block: block.id,
                        kind: stmt.kind.clone(),
                    },
                );

                // Support multiple nodes per line
                line_to_nodes.entry(stmt.line).or_default().push(id);
            }
        }

        // Build edges
        let mut edges: HashMap<u32, Vec<PdgEdge>> = HashMap::new();
        let mut reverse_edges: HashMap<u32, Vec<PdgEdge>> = HashMap::new();

        // Add control dependency edges
        for (&block, deps) in &control_deps.dependencies {
            // Find nodes in this block
            let block_nodes: Vec<u32> = nodes
                .values()
                .filter(|n| n.block == block)
                .map(|n| n.id)
                .collect();

            for &dep_block in deps {
                // Find nodes in the dependency block (condition)
                let dep_nodes: Vec<u32> = nodes
                    .values()
                    .filter(|n| n.block == dep_block)
                    .map(|n| n.id)
                    .collect();

                // Add edges from condition to dependent statements
                for &from in &dep_nodes {
                    for &to in &block_nodes {
                        let edge = PdgEdge {
                            from,
                            to,
                            kind: PdgEdgeKind::Control,
                        };
                        edges.entry(from).or_default().push(edge.clone());
                        reverse_edges.entry(to).or_default().push(edge);
                    }
                }
            }
        }

        // Add data dependency edges
        // Now handles multiple nodes per line
        for (def, uses) in &data_deps.def_use {
            if let Some(from_ids) = line_to_nodes.get(&def.line) {
                for &from_id in from_ids {
                    for u in uses {
                        if let Some(to_ids) = line_to_nodes.get(&u.line) {
                            for &to_id in to_ids {
                                if from_id != to_id {
                                    let edge = PdgEdge {
                                        from: from_id,
                                        to: to_id,
                                        kind: PdgEdgeKind::Data {
                                            variable: def.name.clone(),
                                        },
                                    };
                                    edges.entry(from_id).or_default().push(edge.clone());
                                    reverse_edges.entry(to_id).or_default().push(edge);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            nodes,
            edges,
            reverse_edges,
            line_to_nodes,
            function_name: cfg.function_name.clone(),
            file_path: None,
            cfg,
            data_deps,
            control_deps,
        }
    }

    /// Set the file path
    pub fn with_file_path(mut self, path: PathBuf) -> Self {
        self.file_path = Some(path);
        self
    }

    /// Get a node by ID
    pub fn get_node(&self, id: u32) -> Option<&PdgNode> {
        self.nodes.get(&id)
    }

    /// Get a node by line number (returns first node if multiple exist on same line)
    pub fn get_node_by_line(&self, line: usize) -> Option<&PdgNode> {
        self.line_to_nodes
            .get(&line)
            .and_then(|ids| ids.first())
            .and_then(|id| self.nodes.get(id))
    }

    /// Get all nodes on a given line
    pub fn get_nodes_by_line(&self, line: usize) -> Vec<&PdgNode> {
        self.line_to_nodes
            .get(&line)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get forward dependencies of a node
    pub fn get_dependencies(&self, id: u32) -> Vec<&PdgEdge> {
        self.edges
            .get(&id)
            .map(|e| e.iter().collect())
            .unwrap_or_default()
    }

    /// Get backward dependencies of a node (what this node depends on)
    pub fn get_dependents(&self, id: u32) -> Vec<&PdgEdge> {
        self.reverse_edges
            .get(&id)
            .map(|e| e.iter().collect())
            .unwrap_or_default()
    }

    /// Get all nodes as a sorted list
    pub fn all_nodes(&self) -> Vec<&PdgNode> {
        let mut nodes: Vec<_> = self.nodes.values().collect();
        nodes.sort_by_key(|n| n.line);
        nodes
    }
}
