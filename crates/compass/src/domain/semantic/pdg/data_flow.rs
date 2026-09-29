//! Data Flow Analysis for PDG
//!
//! Implements reaching definitions and def-use chain tracking
//! for data dependency analysis.

mod analyzer;
#[cfg(test)]
mod tests;

use super::cfg::{BlockId, ControlFlowGraph};
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::type_inference::Span;
use std::collections::{HashMap, HashSet};

use analyzer::DataFlowAnalyzer;

/// A variable definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Definition {
    /// Variable name
    pub name: String,
    /// Location in source
    pub span: Span,
    /// Line number
    pub line: usize,
    /// Block ID in CFG
    pub block: BlockId,
    /// Statement index within block
    pub stmt_index: usize,
}

/// A variable use
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Use {
    /// Variable name
    pub name: String,
    /// Location in source
    pub span: Span,
    /// Line number
    pub line: usize,
    /// Block ID in CFG
    pub block: BlockId,
    /// Statement index within block
    pub stmt_index: usize,
}

/// A def-use chain: connects a definition to its uses
#[derive(Debug, Clone)]
pub struct DefUseChain {
    /// The definition
    pub def: Definition,
    /// All uses reached by this definition
    pub uses: Vec<Use>,
}

/// A use-def chain: connects a use to its possible definitions
#[derive(Debug, Clone)]
pub struct UseDefChain {
    /// The use
    pub use_site: Use,
    /// All definitions that may reach this use
    pub defs: Vec<Definition>,
}

/// Data dependencies between statements
#[derive(Debug, Clone)]
pub struct DataDependencies {
    /// Def-use chains: definition -> uses
    pub def_use: HashMap<Definition, Vec<Use>>,
    /// Use-def chains: use -> definitions
    pub use_def: HashMap<Use, Vec<Definition>>,
    /// All definitions by variable name
    pub defs_by_name: HashMap<String, Vec<Definition>>,
    /// All uses by variable name
    pub uses_by_name: HashMap<String, Vec<Use>>,
    /// Reaching definitions at each block entry
    pub reaching_in: HashMap<BlockId, HashSet<Definition>>,
    /// Reaching definitions at each block exit
    pub reaching_out: HashMap<BlockId, HashSet<Definition>>,
}

impl DataDependencies {
    /// Compute data dependencies from CFG and source
    pub fn compute(cfg: &ControlFlowGraph, file: &ParsedFile) -> Self {
        let analyzer = DataFlowAnalyzer::new(cfg, file);
        analyzer.analyze()
    }

    /// Get all definitions that reach a use
    pub fn get_reaching_defs(&self, use_site: &Use) -> Vec<&Definition> {
        self.use_def
            .get(use_site)
            .map(|defs| defs.iter().collect())
            .unwrap_or_default()
    }

    /// Get all uses of a definition
    pub fn get_uses(&self, def: &Definition) -> Vec<&Use> {
        self.def_use
            .get(def)
            .map(|uses| uses.iter().collect())
            .unwrap_or_default()
    }

    /// Get all definitions of a variable
    pub fn get_defs(&self, name: &str) -> Vec<&Definition> {
        self.defs_by_name
            .get(name)
            .map(|defs| defs.iter().collect())
            .unwrap_or_default()
    }

    /// Check if there's a data dependency between two statements
    pub fn has_dependency(&self, from_line: usize, to_line: usize) -> bool {
        for (def, uses) in &self.def_use {
            if def.line == from_line {
                for u in uses {
                    if u.line == to_line {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get all data dependencies for a statement (line)
    pub fn get_dependencies_for_line(&self, line: usize) -> Vec<usize> {
        let mut deps = HashSet::new();

        // Find uses on this line and get their definitions
        for (use_site, defs) in &self.use_def {
            if use_site.line == line {
                for def in defs {
                    deps.insert(def.line);
                }
            }
        }

        deps.into_iter().collect()
    }

    /// Get all statements that depend on a given line
    pub fn get_dependents_for_line(&self, line: usize) -> Vec<usize> {
        let mut dependents = HashSet::new();

        // Find definitions on this line and get their uses
        for (def, uses) in &self.def_use {
            if def.line == line {
                for u in uses {
                    dependents.insert(u.line);
                }
            }
        }

        dependents.into_iter().collect()
    }
}
