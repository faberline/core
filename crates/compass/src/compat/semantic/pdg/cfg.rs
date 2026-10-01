//! Control Flow Graph (CFG) construction for Python
//!
//! Implements statement-level CFG for Python code analysis.
//! Handles: sequential execution, if-else, loops (for/while),
//! exceptions (try/except/finally), and function calls.

pub use crate::domain::semantic::pdg::cfg::{
    BasicBlock, BlockId, BlockKind, CfgBuilder, CfgEdge, ControlFlowGraph, EdgeKind, StatementInfo,
    StatementKind,
};
