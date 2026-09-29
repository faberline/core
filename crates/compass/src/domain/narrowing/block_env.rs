//! CFG-based type narrowing (R2.1)
//!
//! Integrates the Control Flow Graph from the PDG module with the TypeNarrower
//! to provide flow-sensitive type narrowing. For each basic block in the CFG,
//! computes the set of narrowed types that hold on entry to that block.
//!
//! This gives Pyright-level precision: after `if isinstance(x, Foo):`, the
//! type of `x` inside the block is `Foo`, not `Union[Foo, Bar]`.

use std::collections::HashMap;

use crate::domain::type_system::ty::Type;
use crate::semantic::pdg::cfg::{BlockId, ControlFlowGraph};

/// Narrowed type environment for a single CFG block entry
#[derive(Debug, Clone, Default)]
pub struct BlockNarrowEnv {
    /// Variable -> narrowed type at this block entry
    pub narrowed: HashMap<String, Type>,
}

impl BlockNarrowEnv {
    /// Merge two environments (join at control-flow merge points)
    ///
    /// Conservative: when two branches disagree on a type, takes the union.
    pub fn join(&self, other: &BlockNarrowEnv) -> BlockNarrowEnv {
        let mut merged = self.narrowed.clone();

        for (var, other_ty) in &other.narrowed {
            let entry = merged.entry(var.clone()).or_insert(other_ty.clone());
            if entry != other_ty {
                // Branches disagree — take union (conservative)
                *entry = Type::Union(vec![entry.clone(), other_ty.clone()]);
            }
        }

        BlockNarrowEnv { narrowed: merged }
    }

    /// Get the narrowed type for a variable, if known
    pub fn get(&self, var: &str) -> Option<&Type> {
        self.narrowed.get(var)
    }
}

/// Result of CFG-based narrowing analysis
pub struct CfgNarrowingResult {
    /// Narrowing environment at the entry of each block
    pub block_envs: HashMap<BlockId, BlockNarrowEnv>,
}

impl CfgNarrowingResult {
    /// Get the narrowed type of a variable at a given line
    pub fn type_at_line(&self, line: usize, var: &str, cfg: &ControlFlowGraph) -> Option<&Type> {
        // Find which block contains this line
        for (block_id, block) in &cfg.blocks {
            for stmt in &block.statements {
                if stmt.line == line {
                    return self.block_envs.get(block_id).and_then(|env| env.get(var));
                }
            }
        }
        None
    }

    /// Check if a variable is narrowed in the given block
    pub fn is_narrowed_in_block(&self, block_id: BlockId, var: &str) -> bool {
        self.block_envs
            .get(&block_id)
            .and_then(|env| env.get(var))
            .is_some()
    }
}
