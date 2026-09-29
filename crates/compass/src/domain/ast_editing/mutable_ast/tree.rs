use std::sync::Arc;

use crate::domain::ast_editing::mutable_ast::{MutableNode, NodeId, Span};

// ============================================================================
// Mutable AST Tree
// ============================================================================

/// A mutable AST with snapshot support.
pub struct MutableAst {
    /// Root node
    root: MutableNode,
    /// Next node ID
    next_id: usize,
    /// Snapshots for undo
    snapshots: Vec<MutableNode>,
    /// Current snapshot index
    snapshot_index: usize,
    /// Maximum snapshots to keep
    max_snapshots: usize,
}

impl MutableAst {
    /// Create a new mutable AST.
    pub fn new(root: MutableNode) -> Self {
        Self {
            root,
            next_id: 1,
            snapshots: Vec::new(),
            snapshot_index: 0,
            max_snapshots: 100,
        }
    }

    /// Get the root node.
    pub fn root(&self) -> &MutableNode {
        &self.root
    }

    /// Get mutable root node.
    pub fn root_mut(&mut self) -> &mut MutableNode {
        &mut self.root
    }

    /// Generate a new node ID.
    pub fn new_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Take a snapshot for undo support.
    pub fn snapshot(&mut self) {
        // Truncate any redo history
        self.snapshots.truncate(self.snapshot_index);

        // Add new snapshot
        self.snapshots.push(self.root.clone());
        self.snapshot_index = self.snapshots.len();

        // Limit history size
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
            self.snapshot_index = self.snapshots.len();
        }
    }

    /// Undo to previous snapshot.
    pub fn undo(&mut self) -> bool {
        if self.snapshot_index > 0 {
            // Save current state for redo
            if self.snapshot_index == self.snapshots.len() {
                self.snapshots.push(self.root.clone());
            }
            self.snapshot_index -= 1;
            self.root = self.snapshots[self.snapshot_index].clone();
            true
        } else {
            false
        }
    }

    /// Redo to next snapshot.
    pub fn redo(&mut self) -> bool {
        if self.snapshot_index < self.snapshots.len() - 1 {
            self.snapshot_index += 1;
            self.root = self.snapshots[self.snapshot_index].clone();
            true
        } else {
            false
        }
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        self.snapshot_index > 0
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        self.snapshot_index < self.snapshots.len().saturating_sub(1)
    }

    /// Find a node by ID.
    pub fn find_node(&self, id: NodeId) -> Option<&MutableNode> {
        self.root.find_by_id(id)
    }

    /// Apply an edit operation.
    pub fn apply_edit(&mut self, edit: AstEdit) -> bool {
        match edit {
            AstEdit::Replace { target, new_node } => self.replace_node(target, new_node),
            AstEdit::Insert {
                parent,
                index,
                node,
            } => self.insert_node(parent, index, node),
            AstEdit::Remove { target } => self.remove_node(target),
            AstEdit::UpdateValue { target, value } => self.update_value(target, value),
        }
    }

    fn replace_node(&mut self, target: NodeId, new_node: MutableNode) -> bool {
        self.replace_in_subtree(&mut self.root.clone(), target, new_node)
            .map(|new_root| self.root = new_root)
            .is_some()
    }

    fn replace_in_subtree(
        &self,
        node: &mut MutableNode,
        target: NodeId,
        new_node: MutableNode,
    ) -> Option<MutableNode> {
        if node.id == target {
            return Some(new_node);
        }

        let children = Arc::make_mut(&mut node.children);
        for i in 0..children.len() {
            if children[i].id == target {
                children[i] = new_node;
                return Some(node.clone());
            }
            if let Some(updated) =
                self.replace_in_subtree(&mut children[i].clone(), target, new_node.clone())
            {
                children[i] = updated;
                return Some(node.clone());
            }
        }
        None
    }

    fn insert_node(&mut self, parent: NodeId, index: usize, node: MutableNode) -> bool {
        if let Some(parent_node) = self.find_node_mut(parent) {
            parent_node.insert_child(index, node);
            true
        } else {
            false
        }
    }

    fn remove_node(&mut self, target: NodeId) -> bool {
        self.remove_from_subtree(&mut self.root.clone(), target)
            .map(|new_root| self.root = new_root)
            .is_some()
    }

    fn remove_from_subtree(&self, node: &mut MutableNode, target: NodeId) -> Option<MutableNode> {
        let children = Arc::make_mut(&mut node.children);
        for i in 0..children.len() {
            if children[i].id == target {
                children.remove(i);
                return Some(node.clone());
            }
            if let Some(updated) = self.remove_from_subtree(&mut children[i].clone(), target) {
                children[i] = updated;
                return Some(node.clone());
            }
        }
        None
    }

    fn update_value(&mut self, target: NodeId, value: String) -> bool {
        if let Some(node) = self.find_node_mut(target) {
            node.value = Some(value);
            true
        } else {
            false
        }
    }

    fn find_node_mut(&mut self, id: NodeId) -> Option<&mut MutableNode> {
        Self::find_in_subtree_mut(&mut self.root, id)
    }

    fn find_in_subtree_mut(node: &mut MutableNode, target: NodeId) -> Option<&mut MutableNode> {
        if node.id == target {
            return Some(node);
        }
        let children = Arc::make_mut(&mut node.children);
        for child in children.iter_mut() {
            if let Some(found) = Self::find_in_subtree_mut(child, target) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node at a specific span.
    pub fn find_at_span(&self, span: &Span) -> Option<&MutableNode> {
        self.root.find_at_span(span)
    }

    /// Find all nodes of a specific kind.
    pub fn find_by_kind(&self, kind: &str) -> Vec<&MutableNode> {
        let mut results = Vec::new();
        self.root.collect_by_kind(kind, &mut results);
        results
    }

    /// Find the innermost node containing a position.
    pub fn find_at_position(&self, line: usize, col: usize) -> Option<&MutableNode> {
        self.root.find_at_position(line, col)
    }
}

/// An edit operation on the AST.
#[derive(Debug, Clone)]
pub enum AstEdit {
    /// Replace a node with a new one
    Replace {
        target: NodeId,
        new_node: MutableNode,
    },
    /// Insert a new child node
    Insert {
        parent: NodeId,
        index: usize,
        node: MutableNode,
    },
    /// Remove a node
    Remove { target: NodeId },
    /// Update a leaf node's value
    UpdateValue { target: NodeId, value: String },
}
