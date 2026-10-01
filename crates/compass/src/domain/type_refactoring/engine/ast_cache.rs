use std::path::PathBuf;
use std::sync::Arc;

use tree_sitter::Node;

use crate::domain::ast_editing::mutable_ast::{
    MutableAst, MutableNode, NodeId, NodeMetadata, Span,
};
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::syntax::MultiParser;

impl RefactoringEngine {
    /// Populate AST cache for a file.
    pub fn populate_ast_cache(&mut self, file: &PathBuf, content: &str) -> Result<(), String> {
        // Detect language from file extension
        let language = MultiParser::detect_language(file)
            .ok_or_else(|| format!("Failed to detect language for file: {:?}", file))?;

        // Parse the file
        let mut parser =
            MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;
        let parsed = parser
            .parse(content, language)
            .ok_or_else(|| "Failed to parse file".to_string())?;

        // Convert tree-sitter AST to MutableAst
        let mut next_id = 0;
        let root = self.convert_node_to_mutable(&parsed.tree.root_node(), content, &mut next_id);
        let ast = MutableAst::new(root);

        self.ast_cache.insert(file.clone(), ast);
        Ok(())
    }

    /// Convert a tree-sitter Node to MutableNode recursively.
    fn convert_node_to_mutable(
        &self,
        node: &Node,
        source: &str,
        next_id: &mut usize,
    ) -> MutableNode {
        let id = NodeId(*next_id);
        *next_id += 1;

        let kind = node.kind().to_string();
        let span = Span::with_lines(
            node.start_byte(),
            node.end_byte(),
            node.start_position().row,
            node.start_position().column,
            node.end_position().row,
            node.end_position().column,
        );

        // Get node text value for leaf nodes
        let value = if node.child_count() == 0 {
            Some(source[node.byte_range()].to_string())
        } else {
            None
        };

        // Recursively convert children
        let mut children = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            children.push(self.convert_node_to_mutable(&child, source, next_id));
        }

        MutableNode {
            id,
            kind,
            span,
            value,
            children: Arc::new(children),
            metadata: NodeMetadata::default(),
        }
    }

    /// Get AST for a file, loading it if not cached.
    pub fn get_ast(&mut self, file: &PathBuf, content: &str) -> Result<&MutableAst, String> {
        if !self.ast_cache.contains_key(file) {
            self.populate_ast_cache(file, content)?;
        }
        self.ast_cache
            .get(file)
            .ok_or_else(|| format!("AST not found in cache for {}", file.display()))
    }

    /// Get mutable AST for a file, loading it if not cached.
    pub fn get_ast_mut(
        &mut self,
        file: &PathBuf,
        content: &str,
    ) -> Result<&mut MutableAst, String> {
        if !self.ast_cache.contains_key(file) {
            self.populate_ast_cache(file, content)?;
        }
        self.ast_cache
            .get_mut(file)
            .ok_or_else(|| format!("AST not found in cache for {}", file.display()))
    }
}
