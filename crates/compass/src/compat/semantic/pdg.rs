//! Program Dependence Graph (PDG) for Python
//!
//! Combines control dependencies and data dependencies into a unified
//! graph for program analysis. Supports:
//! - Forward and backward slicing
//! - Impact analysis
//! - Taint tracking
//! - Dead code detection

pub mod cfg;
pub mod data_flow;
pub mod dominator;

pub use crate::domain::semantic::pdg::{
    detect_taint_sinks, detect_taint_sources, DeadCodeAnalysis, DependencyReason, DetectedSink,
    DetectedSource, ImpactAnalysis, ImpactAnalysisTree, ImpactTreeNode, PdgEdge, PdgEdgeJson,
    PdgEdgeKind, PdgJson, PdgNode, PdgNodeJson, PdgStats, PdgStatsJson, ProgramDependenceGraph,
    ProgramSlice, SemanticTaintAnalysis, SliceDirection, TaintAnalysis, TaintPath, TaintSinkKind,
    TaintSourceKind,
};
pub use cfg::{BlockId, CfgBuilder, ControlFlowGraph, StatementInfo, StatementKind};
pub use data_flow::{DataDependencies, DefUseChain, Definition, Use, UseDefChain};
pub use dominator::{ControlDependencies, DominatorTree};
