use super::CfgBuilder;
use crate::domain::semantic::pdg::cfg::{BlockKind, EdgeKind};
use crate::domain::syntax::parsed_file::ParsedFile;

impl CfgBuilder<'_> {
    /// Visit an if statement
    pub(super) fn visit_if(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        // Create condition block
        let cond_block = self.cfg.create_block(BlockKind::IfCondition);
        self.cfg
            .add_edge(self.current_block, cond_block, EdgeKind::Sequential);

        // Add condition as statement
        if let Some(condition) = node.child_by_field_name("condition") {
            if let Some(stmt) = self.make_statement_info(&condition, file) {
                self.cfg.add_statement(cond_block, stmt);
            }
        }

        // Create then block
        let then_block = self.cfg.create_block(BlockKind::Normal);
        self.cfg
            .add_edge(cond_block, then_block, EdgeKind::TrueBranch);

        // Create join block (after if/else)
        let join_block = self.cfg.create_block(BlockKind::Normal);

        // Visit then block
        self.current_block = then_block;
        if let Some(consequence) = node.child_by_field_name("consequence") {
            self.visit_block(&consequence, file);
        }

        // Connect then block to join (if no return/break)
        if self.cfg.get_successors(self.current_block).is_empty() {
            self.cfg
                .add_edge(self.current_block, join_block, EdgeKind::Sequential);
        }

        // Handle elif/else clauses
        let mut cursor = node.walk();
        let mut has_else = false;
        let mut last_false_block = cond_block;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "elif_clause" => {
                    // Create elif condition block
                    let elif_cond = self.cfg.create_block(BlockKind::IfCondition);
                    self.cfg
                        .add_edge(last_false_block, elif_cond, EdgeKind::FalseBranch);

                    // Add elif condition
                    if let Some(condition) = child.child_by_field_name("condition") {
                        if let Some(stmt) = self.make_statement_info(&condition, file) {
                            self.cfg.add_statement(elif_cond, stmt);
                        }
                    }

                    // Create elif body block
                    let elif_body = self.cfg.create_block(BlockKind::Normal);
                    self.cfg
                        .add_edge(elif_cond, elif_body, EdgeKind::TrueBranch);

                    // Visit elif body
                    self.current_block = elif_body;
                    if let Some(consequence) = child.child_by_field_name("consequence") {
                        self.visit_block(&consequence, file);
                    }

                    // Connect elif body to join
                    if self.cfg.get_successors(self.current_block).is_empty() {
                        self.cfg
                            .add_edge(self.current_block, join_block, EdgeKind::Sequential);
                    }

                    last_false_block = elif_cond;
                }
                "else_clause" => {
                    has_else = true;

                    // Create else block
                    let else_block = self.cfg.create_block(BlockKind::Normal);
                    self.cfg
                        .add_edge(last_false_block, else_block, EdgeKind::FalseBranch);

                    // Visit else body
                    self.current_block = else_block;
                    if let Some(body) = child.child_by_field_name("body") {
                        self.visit_block(&body, file);
                    } else {
                        // Else clause might have body as direct children
                        self.visit_block(&child, file);
                    }

                    // Connect else to join
                    if self.cfg.get_successors(self.current_block).is_empty() {
                        self.cfg
                            .add_edge(self.current_block, join_block, EdgeKind::Sequential);
                    }
                }
                _ => {}
            }
        }

        // If no else, connect last condition to join via false branch
        if !has_else {
            self.cfg
                .add_edge(last_false_block, join_block, EdgeKind::FalseBranch);
        }

        self.current_block = join_block;
    }
}
