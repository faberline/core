use std::collections::{HashMap, HashSet};

use crate::domain::rust_type_system::lifetimes::{LifetimeError, LifetimeErrorKind};
use crate::domain::rust_type_system::types::LifetimeId;

/// A borrow of a value
#[derive(Debug, Clone)]
pub struct Borrow {
    /// Unique identifier for this borrow
    pub id: BorrowId,
    /// The lifetime of the borrow
    pub lifetime: LifetimeId,
    /// Whether this is a mutable borrow
    pub is_mutable: bool,
    /// The path being borrowed (e.g., "x", "x.field", "*x")
    pub path: String,
    /// Source location of the borrow
    pub span: (usize, usize),
}

/// Unique identifier for borrows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BorrowId(pub usize);

// ============================================================================
// Borrow State
// ============================================================================

/// Tracks active borrows in a scope
#[derive(Debug, Clone, Default)]
pub struct BorrowState {
    /// Active borrows by path
    borrows: HashMap<String, Vec<Borrow>>,
    /// Moved values
    moved: HashSet<String>,
    /// Borrow ID counter
    next_borrow_id: usize,
}

impl BorrowState {
    /// Create a new borrow state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a borrow
    pub fn borrow(
        &mut self,
        path: String,
        lifetime: LifetimeId,
        is_mutable: bool,
        span: (usize, usize),
    ) -> Result<BorrowId, LifetimeError> {
        // Check if value was moved
        if self.moved.contains(&path) {
            return Err(LifetimeError {
                message: format!("Cannot borrow `{}` after move", path),
                kind: LifetimeErrorKind::BorrowOfMovedValue,
                span: Some(span),
                related_spans: vec![],
            });
        }

        // Check for conflicting borrows
        if let Some(existing) = self.borrows.get(&path) {
            for borrow in existing {
                if is_mutable || borrow.is_mutable {
                    // Mutable borrow conflicts with any other borrow
                    return Err(LifetimeError {
                        message: format!(
                            "Cannot borrow `{}` as {} because it is already borrowed as {}",
                            path,
                            if is_mutable { "mutable" } else { "immutable" },
                            if borrow.is_mutable {
                                "mutable"
                            } else {
                                "immutable"
                            }
                        ),
                        kind: LifetimeErrorKind::ConflictingBorrows,
                        span: Some(span),
                        related_spans: vec![borrow.span],
                    });
                }
            }
        }

        let id = BorrowId(self.next_borrow_id);
        self.next_borrow_id += 1;

        let borrow = Borrow {
            id,
            lifetime,
            is_mutable,
            path: path.clone(),
            span,
        };

        self.borrows.entry(path).or_default().push(borrow);
        Ok(id)
    }

    /// End a borrow (when lifetime ends)
    pub fn end_borrow(&mut self, id: BorrowId) {
        for borrows in self.borrows.values_mut() {
            borrows.retain(|b| b.id != id);
        }
    }

    /// Record a move
    pub fn move_value(&mut self, path: &str, span: (usize, usize)) -> Result<(), LifetimeError> {
        // Check if there are active borrows
        if let Some(borrows) = self.borrows.get(path) {
            if !borrows.is_empty() {
                return Err(LifetimeError {
                    message: format!("Cannot move `{}` while borrowed", path),
                    kind: LifetimeErrorKind::CannotMoveFromBorrow,
                    span: Some(span),
                    related_spans: borrows.iter().map(|b| b.span).collect(),
                });
            }
        }

        self.moved.insert(path.to_string());
        Ok(())
    }

    /// Check if a value is still valid (not moved)
    pub fn is_valid(&self, path: &str) -> bool {
        !self.moved.contains(path)
    }

    /// Check if a path has any active mutable borrows
    pub fn has_mutable_borrow(&self, path: &str) -> bool {
        self.borrows
            .get(path)
            .map(|bs| bs.iter().any(|b| b.is_mutable))
            .unwrap_or(false)
    }

    /// Check if a path has any active borrows
    pub fn has_any_borrow(&self, path: &str) -> bool {
        self.borrows
            .get(path)
            .map(|bs| !bs.is_empty())
            .unwrap_or(false)
    }

    /// Clear all borrows (for new scope)
    pub fn clear(&mut self) {
        self.borrows.clear();
        self.moved.clear();
    }

    /// Create a child scope state
    pub fn child(&self) -> Self {
        Self {
            borrows: self.borrows.clone(),
            moved: self.moved.clone(),
            next_borrow_id: self.next_borrow_id,
        }
    }
}
