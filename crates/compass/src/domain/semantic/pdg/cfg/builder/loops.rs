use super::{CfgBuilder, LoopContext};
use crate::domain::semantic::pdg::cfg::{BlockKind, EdgeKind};
use crate::domain::syntax::parsed_file::ParsedFile;

impl CfgBuilder<'_> {
    /// Visit a for loop
    pub(super) fn visit_for(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        // Create condition block (iterator check)
        let cond_block = self.cfg.create_block(BlockKind::LoopCondition);
        self.cfg
            .add_edge(self.current_block, cond_block, EdgeKind::Sequential);

        // Add loop header as statement
        if let Some(stmt) = self.make_statement_info(node, file) {
            self.cfg.add_statement(cond_block, stmt);
        }

        // Create body block
        let body_block = self.cfg.create_block(BlockKind::Normal);
        self.cfg
            .add_edge(cond_block, body_block, EdgeKind::TrueBranch);

        // Check if there's an else clause
        let mut cursor = node.walk();
        let mut else_clause = None;
        for child in node.children(&mut cursor) {
            if child.kind() == "else_clause" {
                else_clause = Some(child);
                break;
            }
        }

        // Create else block if present, otherwise use exit directly
        let (else_block, exit_block) = if else_clause.is_some() {
            let else_blk = self.cfg.create_block(BlockKind::Normal);
            let exit_blk = self.cfg.create_block(BlockKind::Normal);
            // False branch goes to else block (loop completed without break)
            self.cfg
                .add_edge(cond_block, else_blk, EdgeKind::FalseBranch);
            (Some(else_blk), exit_blk)
        } else {
            let exit_blk = self.cfg.create_block(BlockKind::Normal);
            // False branch goes directly to exit
            self.cfg
                .add_edge(cond_block, exit_blk, EdgeKind::FalseBranch);
            (None, exit_blk)
        };

        // Push loop context - break goes to exit (skipping else)
        self.loop_stack.push(LoopContext {
            condition: cond_block,
            exit: exit_block,
        });

        // Visit body
        self.current_block = body_block;
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(&body, file);
        }

        // Connect end of body back to condition
        if self.cfg.get_successors(self.current_block).is_empty() {
            self.cfg
                .add_edge(self.current_block, cond_block, EdgeKind::LoopBack);
        }

        // Visit else clause if present
        if let (Some(else_blk), Some(else_node)) = (else_block, else_clause) {
            self.current_block = else_blk;
            if let Some(body) = else_node.child_by_field_name("body") {
                self.visit_block(&body, file);
            } else {
                // Else clause body might be direct children
                self.visit_block(&else_node, file);
            }
            // Connect else to exit
            if self.cfg.get_successors(self.current_block).is_empty() {
                self.cfg
                    .add_edge(self.current_block, exit_block, EdgeKind::Sequential);
            }
        }

        // Pop loop context
        self.loop_stack.pop();

        self.current_block = exit_block;
    }

    /// Visit a while loop
    pub(super) fn visit_while(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        // Create condition block
        let cond_block = self.cfg.create_block(BlockKind::LoopCondition);
        self.cfg
            .add_edge(self.current_block, cond_block, EdgeKind::Sequential);

        // Add condition as statement
        if let Some(condition) = node.child_by_field_name("condition") {
            if let Some(stmt) = self.make_statement_info(&condition, file) {
                self.cfg.add_statement(cond_block, stmt);
            }
        }

        // Create body and exit blocks
        let body_block = self.cfg.create_block(BlockKind::Normal);
        let exit_block = self.cfg.create_block(BlockKind::Normal);

        self.cfg
            .add_edge(cond_block, body_block, EdgeKind::TrueBranch);
        self.cfg
            .add_edge(cond_block, exit_block, EdgeKind::FalseBranch);

        // Push loop context
        self.loop_stack.push(LoopContext {
            condition: cond_block,
            exit: exit_block,
        });

        // Visit body
        self.current_block = body_block;
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(&body, file);
        }

        // Connect end of body back to condition
        if self.cfg.get_successors(self.current_block).is_empty() {
            self.cfg
                .add_edge(self.current_block, cond_block, EdgeKind::LoopBack);
        }

        // Pop loop context
        self.loop_stack.pop();

        self.current_block = exit_block;
    }
}
