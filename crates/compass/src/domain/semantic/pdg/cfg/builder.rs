mod branch;
mod loops;
mod try_except;

use crate::domain::semantic::pdg::cfg::{
    BlockId, BlockKind, ControlFlowGraph, EdgeKind, StatementInfo, StatementKind,
};
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::type_inference::Span;

/// CFG builder for Python code
pub struct CfgBuilder<'a> {
    /// The CFG being built
    cfg: ControlFlowGraph,
    /// Source code (reserved for future use in statement extraction)
    _source: &'a str,
    /// Current block being built
    current_block: BlockId,
    /// Loop stack (for break/continue handling)
    loop_stack: Vec<LoopContext>,
    /// Try stack (for exception handling)
    try_stack: Vec<TryContext>,
}

/// Context for a loop
struct LoopContext {
    /// Condition block (for continue)
    condition: BlockId,
    /// Exit block (for break)
    exit: BlockId,
}

/// Context for a try block
struct TryContext {
    /// Except handlers
    handlers: Vec<BlockId>,
    /// Finally block (if any)
    _finally: Option<BlockId>,
}

impl<'a> CfgBuilder<'a> {
    /// Create a new CFG builder
    pub fn new(source: &'a str) -> Self {
        let cfg = ControlFlowGraph::new();
        Self {
            current_block: cfg.entry,
            cfg,
            _source: source,
            loop_stack: Vec::new(),
            try_stack: Vec::new(),
        }
    }

    /// Build CFG from a parsed file
    pub fn build(mut self, file: &ParsedFile) -> ControlFlowGraph {
        let root = file.root_node();

        // Create initial block for module body
        let first_block = self.cfg.create_block(BlockKind::Normal);
        self.cfg
            .add_edge(self.cfg.entry, first_block, EdgeKind::Sequential);
        self.current_block = first_block;

        // Visit all top-level statements
        self.visit_block(&root, file);

        // Connect last block to exit if not already connected
        if self.cfg.get_successors(self.current_block).is_empty() {
            self.cfg
                .add_edge(self.current_block, self.cfg.exit, EdgeKind::Sequential);
        }

        self.cfg
    }

    /// Build CFG for a specific function
    pub fn build_function(
        mut self,
        func_node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
    ) -> ControlFlowGraph {
        // Get function name
        if let Some(name_node) = func_node.child_by_field_name("name") {
            self.cfg.function_name = Some(file.node_text(&name_node).to_string());
        }

        // Create initial block for function body
        let first_block = self.cfg.create_block(BlockKind::Normal);
        self.cfg
            .add_edge(self.cfg.entry, first_block, EdgeKind::Sequential);
        self.current_block = first_block;

        // Visit function body
        if let Some(body) = func_node.child_by_field_name("body") {
            self.visit_block(&body, file);
        }

        // Connect last block to exit if not already connected
        if self.cfg.get_successors(self.current_block).is_empty() {
            self.cfg
                .add_edge(self.current_block, self.cfg.exit, EdgeKind::Sequential);
        }

        self.cfg
    }

    /// Visit a block of statements
    fn visit_block(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_statement(&child, file);
        }
    }

    /// Visit a single statement
    fn visit_statement(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        match node.kind() {
            "if_statement" => self.visit_if(node, file),
            "for_statement" => self.visit_for(node, file),
            "while_statement" => self.visit_while(node, file),
            "try_statement" => self.visit_try(node, file),
            "return_statement" => self.visit_return(node, file),
            "break_statement" => self.visit_break(node, file),
            "continue_statement" => self.visit_continue(node, file),
            "raise_statement" => self.visit_raise(node, file),
            "function_definition" | "async_function_definition" => {
                // Skip nested functions for now (handled separately)
            }
            "class_definition" => {
                // Skip class definitions for now
            }
            _ => {
                // Regular statement - add to current block
                if let Some(stmt) = self.make_statement_info(node, file) {
                    self.cfg.add_statement(self.current_block, stmt);
                }
            }
        }
    }

    /// Visit a return statement
    fn visit_return(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if let Some(stmt) = self.make_statement_info(node, file) {
            self.cfg.add_statement(self.current_block, stmt);
        }
        self.cfg
            .add_edge(self.current_block, self.cfg.exit, EdgeKind::Return);
    }

    /// Visit a break statement
    fn visit_break(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if let Some(stmt) = self.make_statement_info(node, file) {
            self.cfg.add_statement(self.current_block, stmt);
        }
        if let Some(loop_ctx) = self.loop_stack.last() {
            self.cfg
                .add_edge(self.current_block, loop_ctx.exit, EdgeKind::Break);
        }
    }

    /// Visit a continue statement
    fn visit_continue(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if let Some(stmt) = self.make_statement_info(node, file) {
            self.cfg.add_statement(self.current_block, stmt);
        }
        if let Some(loop_ctx) = self.loop_stack.last() {
            self.cfg
                .add_edge(self.current_block, loop_ctx.condition, EdgeKind::Continue);
        }
    }

    /// Visit a raise statement
    fn visit_raise(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if let Some(stmt) = self.make_statement_info(node, file) {
            self.cfg.add_statement(self.current_block, stmt);
        }

        // Connect to exception handlers if in try block
        if let Some(try_ctx) = self.try_stack.last() {
            for &handler in &try_ctx.handlers {
                self.cfg
                    .add_edge(self.current_block, handler, EdgeKind::Exception);
            }
        } else {
            // Uncaught exception goes to exit
            self.cfg
                .add_edge(self.current_block, self.cfg.exit, EdgeKind::Exception);
        }
    }

    /// Create a StatementInfo from a node
    fn make_statement_info(
        &self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
    ) -> Option<StatementInfo> {
        let kind = match node.kind() {
            "assignment" => StatementKind::Assignment,
            "augmented_assignment" => StatementKind::AugmentedAssignment,
            "expression_statement" => StatementKind::Expression,
            "return_statement" => StatementKind::Return,
            "raise_statement" => StatementKind::Raise,
            "assert_statement" => StatementKind::Assert,
            "pass_statement" => StatementKind::Pass,
            "break_statement" => StatementKind::Break,
            "continue_statement" => StatementKind::Continue,
            "import_statement" | "import_from_statement" => StatementKind::Import,
            "global_statement" | "nonlocal_statement" => StatementKind::Declaration,
            "delete_statement" => StatementKind::Delete,
            // Skip control flow structures (handled separately)
            "if_statement" | "for_statement" | "while_statement" | "try_statement" => return None,
            // Skip definitions
            "function_definition" | "async_function_definition" | "class_definition" => {
                return None
            }
            _ => StatementKind::Other,
        };

        Some(StatementInfo {
            span: Span {
                start: node.start_byte(),
                end: node.end_byte(),
                start_line: node.start_position().row,
                start_col: node.start_position().column,
                end_line: node.end_position().row,
                end_col: node.end_position().column,
            },
            kind,
            line: node.start_position().row,
            text: file.node_text(node).to_string(),
        })
    }
}
