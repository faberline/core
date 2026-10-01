use crate::domain::diagnostic::model::{Position, Range};
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parse_error::ParseError;
use tree_sitter::Tree;

/// Parsed file with AST
pub struct ParsedFile {
    pub source: String,
    pub tree: Tree,
    pub language: Language,
    pub has_errors: bool,
    /// True when the file was created via `line_based()` (no real tree-sitter parse).
    /// Checkers must fall back to line-based analysis in this case even though
    /// `has_errors` is false.
    pub is_line_based: bool,
}

impl ParsedFile {
    /// Get the root node
    pub fn root_node(&self) -> tree_sitter::Node<'_> {
        self.tree.root_node()
    }

    /// Get source text for a node
    pub fn node_text(&self, node: &tree_sitter::Node<'_>) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Walk the AST with a visitor function
    /// Returns true to continue traversal, false to stop
    pub fn walk<F>(&self, mut visitor: F)
    where
        F: FnMut(&tree_sitter::Node<'_>, usize) -> bool,
    {
        Self::walk_recursive(&self.root_node(), 0, &mut visitor);
    }

    fn walk_recursive<F>(node: &tree_sitter::Node<'_>, depth: usize, visitor: &mut F)
    where
        F: FnMut(&tree_sitter::Node<'_>, usize) -> bool,
    {
        if !visitor(node, depth) {
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_recursive(&child, depth + 1, visitor);
        }
    }

    /// Collect all parse errors from the tree
    pub fn collect_errors(&self) -> Vec<ParseError> {
        let mut errors = Vec::new();
        self.walk(|node, _depth| {
            if node.is_error() || node.is_missing() {
                errors.push(ParseError {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    start_position: (
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    ),
                    end_position: (node.end_position().row + 1, node.end_position().column + 1),
                    kind: node.kind().to_string(),
                });
            }
            true
        });
        errors
    }

    /// Walk the AST with error recovery, skipping ERROR nodes
    pub fn walk_with_recovery<F>(&self, mut visitor: F)
    where
        F: FnMut(&tree_sitter::Node<'_>, usize) -> bool,
    {
        Self::walk_with_recovery_recursive(&self.root_node(), 0, &mut visitor);
    }

    fn walk_with_recovery_recursive<F>(node: &tree_sitter::Node<'_>, depth: usize, visitor: &mut F)
    where
        F: FnMut(&tree_sitter::Node<'_>, usize) -> bool,
    {
        if node.is_error() || node.is_missing() {
            return;
        }

        if !visitor(node, depth) {
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_with_recovery_recursive(&child, depth + 1, visitor);
        }
    }

    /// Get valid (non-error) top-level statements
    pub fn valid_statements(&self) -> Vec<tree_sitter::Node<'_>> {
        let root = self.root_node();
        let mut cursor = root.walk();
        let mut valid = Vec::new();

        for child in root.children(&mut cursor) {
            if !child.is_error() && !child.is_missing() {
                valid.push(child);
            }
        }

        valid
    }

    /// Check if a node is inside an error region
    pub fn is_inside_error(&self, node: &tree_sitter::Node<'_>) -> bool {
        let mut current = *node;
        while let Some(parent) = current.parent() {
            if parent.is_error() {
                return true;
            }
            current = parent;
        }
        false
    }

    /// Get the next valid sibling after an error node
    pub fn synchronize_after<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        let mut current = node.next_sibling();
        while let Some(sibling) = current {
            if !sibling.is_error() && !sibling.is_missing() {
                return Some(sibling);
            }
            current = sibling.next_sibling();
        }
        None
    }
}

/// The source range a syntax node spans.
///
/// Implemented for `tree_sitter::Node` here, next to `ParsedFile`, so the
/// diagnostic model's `Range` does not depend on tree-sitter.
pub trait NodeRange {
    /// The node's start and end, as 0-based lines and columns.
    fn to_range(&self) -> Range;
}

impl NodeRange for tree_sitter::Node<'_> {
    fn to_range(&self) -> Range {
        let start = self.start_position();
        let end = self.end_position();
        Range::new(
            Position::new(start.row as u32, start.column as u32),
            Position::new(end.row as u32, end.column as u32),
        )
    }
}
