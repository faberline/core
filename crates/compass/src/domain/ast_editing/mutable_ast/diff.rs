use crate::domain::ast_editing::mutable_ast::{AstEdit, MutableNode};

// ============================================================================
// Tree Diff
// ============================================================================

/// A diff between two AST trees.
#[derive(Debug, Clone)]
pub struct TreeDiff {
    /// Edit operations to transform old tree to new tree
    pub edits: Vec<AstEdit>,
}

impl TreeDiff {
    /// Compute diff between two trees.
    pub fn compute(old: &MutableNode, new: &MutableNode) -> Self {
        let mut edits = Vec::new();
        Self::compute_diff(old, new, &mut edits);
        Self { edits }
    }

    fn compute_diff(old: &MutableNode, new: &MutableNode, edits: &mut Vec<AstEdit>) {
        // If nodes are different types or values, replace
        if old.kind != new.kind || old.value != new.value {
            edits.push(AstEdit::Replace {
                target: old.id,
                new_node: new.clone(),
            });
            return;
        }

        // Compare children
        let old_len = old.children.len();
        let new_len = new.children.len();

        // Compare existing children
        let min_len = old_len.min(new_len);
        for i in 0..min_len {
            Self::compute_diff(&old.children[i], &new.children[i], edits);
        }

        // Handle removed children
        for i in (new_len..old_len).rev() {
            edits.push(AstEdit::Remove {
                target: old.children[i].id,
            });
        }

        // Handle added children
        for i in old_len..new_len {
            edits.push(AstEdit::Insert {
                parent: old.id,
                index: i,
                node: new.children[i].clone(),
            });
        }
    }

    /// Check if diff is empty.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Get number of edits.
    pub fn len(&self) -> usize {
        self.edits.len()
    }
}
