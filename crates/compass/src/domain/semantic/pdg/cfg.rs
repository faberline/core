//! Control Flow Graph (CFG) construction for Python
//!
//! Implements statement-level CFG for Python code analysis.
//! Handles: sequential execution, if-else, loops (for/while),
//! exceptions (try/except/finally), and function calls.

mod builder;
#[cfg(test)]
mod tests;

use crate::type_inference::Span;
use std::collections::{HashMap, HashSet};

pub use builder::CfgBuilder;

/// Unique identifier for a CFG block
pub type BlockId = u32;

/// A basic block in the CFG
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Unique identifier
    pub id: BlockId,
    /// Statements in this block (as spans into source)
    pub statements: Vec<StatementInfo>,
    /// Block kind for control flow analysis
    pub kind: BlockKind,
}

/// Information about a statement
#[derive(Debug, Clone)]
pub struct StatementInfo {
    /// Span in source code
    pub span: Span,
    /// Statement kind
    pub kind: StatementKind,
    /// Line number (0-indexed)
    pub line: usize,
    /// Source text (for debugging)
    pub text: String,
}

/// Kind of statement
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatementKind {
    /// Variable assignment
    Assignment,
    /// Augmented assignment (+=, -=, etc.)
    AugmentedAssignment,
    /// Expression statement (function call, etc.)
    Expression,
    /// Return statement
    Return,
    /// Raise statement
    Raise,
    /// Assert statement
    Assert,
    /// Pass statement
    Pass,
    /// Break statement
    Break,
    /// Continue statement
    Continue,
    /// Import statement
    Import,
    /// Global/nonlocal declaration
    Declaration,
    /// Delete statement
    Delete,
    /// Other/unknown
    Other,
}

/// Kind of basic block
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    /// Entry block
    Entry,
    /// Exit block
    Exit,
    /// Normal sequential block
    Normal,
    /// If condition block
    IfCondition,
    /// Loop condition block (for/while)
    LoopCondition,
    /// Try block entry
    TryEntry,
    /// Except handler
    ExceptHandler,
    /// Finally block
    Finally,
    /// Function call (for inter-procedural)
    FunctionCall { callee: String },
}

/// Edge type in CFG
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeKind {
    /// Normal sequential flow
    Sequential,
    /// True branch of condition
    TrueBranch,
    /// False branch of condition
    FalseBranch,
    /// Loop back edge
    LoopBack,
    /// Exception edge (to handler)
    Exception,
    /// Break edge (out of loop)
    Break,
    /// Continue edge (back to loop)
    Continue,
    /// Return edge (to exit)
    Return,
}

/// An edge in the CFG
#[derive(Debug, Clone)]
pub struct CfgEdge {
    /// Source block
    pub from: BlockId,
    /// Target block
    pub to: BlockId,
    /// Edge kind
    pub kind: EdgeKind,
}

/// Control Flow Graph for a function/module
#[derive(Debug, Clone)]
pub struct ControlFlowGraph {
    /// All basic blocks
    pub blocks: HashMap<BlockId, BasicBlock>,
    /// Edges from each block
    pub successors: HashMap<BlockId, Vec<CfgEdge>>,
    /// Edges to each block
    pub predecessors: HashMap<BlockId, Vec<CfgEdge>>,
    /// Entry block ID
    pub entry: BlockId,
    /// Exit block ID
    pub exit: BlockId,
    /// Next block ID to allocate
    next_id: BlockId,
    /// Function name (if applicable)
    pub function_name: Option<String>,
}

impl ControlFlowGraph {
    /// Create a new empty CFG
    pub fn new() -> Self {
        let mut cfg = Self {
            blocks: HashMap::new(),
            successors: HashMap::new(),
            predecessors: HashMap::new(),
            entry: 0,
            exit: 1,
            next_id: 2,
            function_name: None,
        };

        // Create entry and exit blocks
        cfg.blocks.insert(
            0,
            BasicBlock {
                id: 0,
                statements: Vec::new(),
                kind: BlockKind::Entry,
            },
        );
        cfg.blocks.insert(
            1,
            BasicBlock {
                id: 1,
                statements: Vec::new(),
                kind: BlockKind::Exit,
            },
        );

        cfg
    }

    /// Create a new basic block
    pub fn create_block(&mut self, kind: BlockKind) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        self.blocks.insert(
            id,
            BasicBlock {
                id,
                statements: Vec::new(),
                kind,
            },
        );
        id
    }

    /// Add an edge between blocks
    pub fn add_edge(&mut self, from: BlockId, to: BlockId, kind: EdgeKind) {
        let edge = CfgEdge {
            from,
            to,
            kind: kind.clone(),
        };
        self.successors.entry(from).or_default().push(edge.clone());
        self.predecessors.entry(to).or_default().push(edge);
    }

    /// Add a statement to a block
    pub fn add_statement(&mut self, block_id: BlockId, stmt: StatementInfo) {
        if let Some(block) = self.blocks.get_mut(&block_id) {
            block.statements.push(stmt);
        }
    }

    /// Get successors of a block
    pub fn get_successors(&self, block_id: BlockId) -> Vec<BlockId> {
        self.successors
            .get(&block_id)
            .map(|edges| edges.iter().map(|e| e.to).collect())
            .unwrap_or_default()
    }

    /// Get predecessors of a block
    pub fn get_predecessors(&self, block_id: BlockId) -> Vec<BlockId> {
        self.predecessors
            .get(&block_id)
            .map(|edges| edges.iter().map(|e| e.from).collect())
            .unwrap_or_default()
    }

    /// Get all block IDs
    pub fn block_ids(&self) -> Vec<BlockId> {
        self.blocks.keys().copied().collect()
    }

    /// Get a block by ID
    pub fn get_block(&self, id: BlockId) -> Option<&BasicBlock> {
        self.blocks.get(&id)
    }

    /// Get all statements across all blocks
    pub fn all_statements(&self) -> Vec<&StatementInfo> {
        let mut stmts = Vec::new();
        for block in self.blocks.values() {
            for stmt in &block.statements {
                stmts.push(stmt);
            }
        }
        stmts.sort_by_key(|s| s.line);
        stmts
    }

    /// Check if block A dominates block B
    /// (A appears on every path from entry to B)
    pub fn dominates(&self, a: BlockId, b: BlockId) -> bool {
        if a == b {
            return true;
        }
        // Use simple BFS to check if B is reachable without going through A
        let mut visited = HashSet::new();
        let mut queue = vec![self.entry];

        while let Some(current) = queue.pop() {
            if current == a {
                continue; // Skip A
            }
            if current == b {
                return false; // Reached B without A
            }
            if visited.insert(current) {
                queue.extend(self.get_successors(current));
            }
        }
        true
    }
}

impl Default for ControlFlowGraph {
    fn default() -> Self {
        Self::new()
    }
}
