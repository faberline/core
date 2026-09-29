//! Mutable AST with persistent data structures (Sprint 2 - Track 2)
//!
//! Provides a mutable AST implementation for efficient code transformations:
//! - Copy-on-write node modifications
//! - Persistent immutable snapshots
//! - Efficient tree diffing
//! - Undo/redo support

use std::collections::HashMap;
use std::sync::Arc;

mod diff;
mod tree;

pub use diff::TreeDiff;
pub use tree::{AstEdit, MutableAst};

// ============================================================================
// Node ID and References
// ============================================================================

/// Unique identifier for AST nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// A reference to a node that may be borrowed or owned.
#[derive(Debug, Clone)]
pub enum NodeRef {
    /// Direct reference by ID
    Direct(NodeId),
    /// Path from root
    Path(Vec<usize>),
}

// ============================================================================
// Mutable AST Node
// ============================================================================

/// A mutable AST node with copy-on-write semantics.
#[derive(Debug, Clone)]
pub struct MutableNode {
    /// Unique node ID
    pub id: NodeId,
    /// Node kind (e.g., "function_definition", "assignment")
    pub kind: String,
    /// Text span in source
    pub span: Span,
    /// Node value/text (for leaf nodes)
    pub value: Option<String>,
    /// Child nodes (copy-on-write)
    pub children: Arc<Vec<MutableNode>>,
    /// Node metadata
    pub metadata: NodeMetadata,
}

/// Span in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// Start column (0-indexed)
    pub start_col: usize,
    /// End line
    pub end_line: usize,
    /// End column
    pub end_col: usize,
}

impl Span {
    /// Create a new span.
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
        }
    }

    /// Create with full position info.
    pub fn with_lines(
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Check if spans overlap.
    pub fn overlaps(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Check if this span contains another.
    pub fn contains(&self, other: &Span) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

/// Metadata attached to nodes.
#[derive(Debug, Clone, Default)]
pub struct NodeMetadata {
    /// Type annotation (if any)
    pub type_annotation: Option<String>,
    /// Documentation comment
    pub docstring: Option<String>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl MutableNode {
    /// Create a new mutable node.
    pub fn new(id: NodeId, kind: impl Into<String>, span: Span) -> Self {
        Self {
            id,
            kind: kind.into(),
            span,
            value: None,
            children: Arc::new(Vec::new()),
            metadata: NodeMetadata::default(),
        }
    }

    /// Create a leaf node with value.
    pub fn leaf(id: NodeId, kind: impl Into<String>, span: Span, value: impl Into<String>) -> Self {
        Self {
            id,
            kind: kind.into(),
            span,
            value: Some(value.into()),
            children: Arc::new(Vec::new()),
            metadata: NodeMetadata::default(),
        }
    }

    /// Add a child node (copy-on-write).
    pub fn add_child(&mut self, child: MutableNode) {
        Arc::make_mut(&mut self.children).push(child);
    }

    /// Get child at index.
    pub fn child(&self, index: usize) -> Option<&MutableNode> {
        self.children.get(index)
    }

    /// Get mutable child at index (copy-on-write).
    pub fn child_mut(&mut self, index: usize) -> Option<&mut MutableNode> {
        Arc::make_mut(&mut self.children).get_mut(index)
    }

    /// Find child by kind.
    pub fn find_child(&self, kind: &str) -> Option<&MutableNode> {
        self.children.iter().find(|c| c.kind == kind)
    }

    /// Replace a child at index.
    pub fn replace_child(&mut self, index: usize, new_child: MutableNode) -> Option<MutableNode> {
        let children = Arc::make_mut(&mut self.children);
        if index < children.len() {
            Some(std::mem::replace(&mut children[index], new_child))
        } else {
            None
        }
    }

    /// Remove a child at index.
    pub fn remove_child(&mut self, index: usize) -> Option<MutableNode> {
        let children = Arc::make_mut(&mut self.children);
        if index < children.len() {
            Some(children.remove(index))
        } else {
            None
        }
    }

    /// Insert a child at index.
    pub fn insert_child(&mut self, index: usize, child: MutableNode) {
        let children = Arc::make_mut(&mut self.children);
        if index <= children.len() {
            children.insert(index, child);
        }
    }

    /// Traverse the tree depth-first.
    pub fn traverse<F>(&self, mut f: F)
    where
        F: FnMut(&MutableNode),
    {
        self.traverse_impl(&mut f);
    }

    fn traverse_impl<F>(&self, f: &mut F)
    where
        F: FnMut(&MutableNode),
    {
        f(self);
        for child in self.children.iter() {
            child.traverse_impl(f);
        }
    }

    /// Find a node by ID.
    pub fn find_by_id(&self, target: NodeId) -> Option<&MutableNode> {
        if self.id == target {
            return Some(self);
        }
        for child in self.children.iter() {
            if let Some(found) = child.find_by_id(target) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node at a specific span.
    pub fn find_at_span(&self, span: &Span) -> Option<&MutableNode> {
        if self.span.start == span.start && self.span.end == span.end {
            return Some(self);
        }
        for child in self.children.iter() {
            if let Some(found) = child.find_at_span(span) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all nodes of a specific kind.
    pub fn collect_by_kind<'a>(&'a self, kind: &str, results: &mut Vec<&'a MutableNode>) {
        if self.kind == kind {
            results.push(self);
        }
        for child in self.children.iter() {
            child.collect_by_kind(kind, results);
        }
    }

    /// Find the innermost node containing a position.
    pub fn find_at_position(&self, line: usize, col: usize) -> Option<&MutableNode> {
        // Check if position is within this node's span
        if line < self.span.start_line || line > self.span.end_line {
            return None;
        }
        if line == self.span.start_line && col < self.span.start_col {
            return None;
        }
        if line == self.span.end_line && col > self.span.end_col {
            return None;
        }

        // Check children for a more specific match
        for child in self.children.iter() {
            if let Some(found) = child.find_at_position(line, col) {
                return Some(found);
            }
        }

        // This is the innermost node containing the position
        Some(self)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
