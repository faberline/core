use super::{CfgBuilder, TryContext};
use crate::domain::semantic::pdg::cfg::{BlockId, BlockKind, EdgeKind};
use crate::domain::syntax::parsed_file::ParsedFile;

impl CfgBuilder<'_> {
    /// Visit a try statement
    ///
    /// Creates exception edges from each block in the try body to handlers,
    /// providing more accurate control flow for exception handling.
    pub(super) fn visit_try(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let try_entry = self.cfg.create_block(BlockKind::TryEntry);
        self.cfg
            .add_edge(self.current_block, try_entry, EdgeKind::Sequential);

        let exit_block = self.cfg.create_block(BlockKind::Normal);
        let mut handler_blocks = Vec::new();
        let mut finally_block = None;

        // First pass: create handler and finally blocks
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "except_clause" => {
                    let handler = self.cfg.create_block(BlockKind::ExceptHandler);
                    handler_blocks.push(handler);
                }
                "finally_clause" => {
                    finally_block = Some(self.cfg.create_block(BlockKind::Finally));
                }
                _ => {}
            }
        }

        // Push try context - this allows raise statements to connect to handlers
        self.try_stack.push(TryContext {
            handlers: handler_blocks.clone(),
            _finally: finally_block,
        });

        // Track blocks created during try body for exception edges
        let blocks_before = self.cfg.blocks.len();

        // Visit try body
        self.current_block = try_entry;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                self.visit_block(&child, file);
                break;
            }
        }

        let try_body_end = self.current_block;

        // Add exception edges from all blocks created in try body to handlers
        // This models that any statement in try could throw
        let blocks_after: Vec<BlockId> = self
            .cfg
            .blocks
            .keys()
            .copied()
            .filter(|&id| {
                // Include try_entry and all blocks created during try body
                id == try_entry
                    || (id >= blocks_before as u32
                        && id != exit_block
                        && !handler_blocks.contains(&id)
                        && Some(id) != finally_block)
            })
            .collect();

        for block_id in blocks_after {
            // Skip if this block already has exception edges (e.g., from raise)
            let has_exception_edge = self
                .cfg
                .successors
                .get(&block_id)
                .map(|edges| edges.iter().any(|e| e.kind == EdgeKind::Exception))
                .unwrap_or(false);

            if !has_exception_edge {
                for &handler in &handler_blocks {
                    self.cfg.add_edge(block_id, handler, EdgeKind::Exception);
                }
            }
        }

        // Connect try body end to finally or exit (normal path)
        if let Some(finally) = finally_block {
            if self.cfg.get_successors(try_body_end).is_empty()
                || !self
                    .cfg
                    .get_successors(try_body_end)
                    .iter()
                    .any(|id| *id == finally)
            {
                self.cfg
                    .add_edge(try_body_end, finally, EdgeKind::Sequential);
            }
        } else if self.cfg.get_successors(try_body_end).is_empty() {
            self.cfg
                .add_edge(try_body_end, exit_block, EdgeKind::Sequential);
        }

        // Visit except handlers
        let mut cursor = node.walk();
        let mut handler_idx = 0;
        for child in node.children(&mut cursor) {
            if child.kind() == "except_clause" && handler_idx < handler_blocks.len() {
                self.current_block = handler_blocks[handler_idx];
                self.visit_block(&child, file);

                // Connect handler to finally or exit
                if let Some(finally) = finally_block {
                    if self.cfg.get_successors(self.current_block).is_empty() {
                        self.cfg
                            .add_edge(self.current_block, finally, EdgeKind::Sequential);
                    }
                } else if self.cfg.get_successors(self.current_block).is_empty() {
                    self.cfg
                        .add_edge(self.current_block, exit_block, EdgeKind::Sequential);
                }

                handler_idx += 1;
            }
        }

        // Visit finally block
        if let Some(finally) = finally_block {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "finally_clause" {
                    self.current_block = finally;
                    self.visit_block(&child, file);

                    // Connect finally to exit
                    if self.cfg.get_successors(self.current_block).is_empty() {
                        self.cfg
                            .add_edge(self.current_block, exit_block, EdgeKind::Sequential);
                    }
                    break;
                }
            }
        }

        // Pop try context
        self.try_stack.pop();

        self.current_block = exit_block;
    }
}
