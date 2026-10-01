//! Rust lifetime analysis
//!
//! This module provides lifetime analysis for Rust code, including:
//! - Lifetime constraint tracking
//! - Borrow checking (mutable vs immutable)
//! - Lifetime error reporting

use std::collections::{HashMap, HashSet};

use crate::domain::rust_type_system::types::{Lifetime, LifetimeId, RustType};

mod borrow;

pub use borrow::{Borrow, BorrowId, BorrowState};

// ============================================================================
// Lifetime Constraints
// ============================================================================

/// A lifetime constraint representing 'a: 'b (a outlives b)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LifetimeConstraint {
    /// The longer lifetime (must outlive `shorter`)
    pub longer: LifetimeId,
    /// The shorter lifetime (must be outlived by `longer`)
    pub shorter: LifetimeId,
    /// Source location of the constraint
    pub span: Option<(usize, usize)>,
}

// ============================================================================
// Lifetime Errors
// ============================================================================

/// Lifetime-related error
#[derive(Debug, Clone)]
pub struct LifetimeError {
    /// Error message
    pub message: String,
    /// Error kind
    pub kind: LifetimeErrorKind,
    /// Source location
    pub span: Option<(usize, usize)>,
    /// Related locations (e.g., conflicting borrow locations)
    pub related_spans: Vec<(usize, usize)>,
}

/// Kind of lifetime error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifetimeErrorKind {
    /// Lifetime does not live long enough
    DoesNotLiveLongEnough,
    /// Conflicting borrows (e.g., mutable + immutable)
    ConflictingBorrows,
    /// Cannot move out of borrowed reference
    CannotMoveFromBorrow,
    /// Value used after move
    UseAfterMove,
    /// Borrow of moved value
    BorrowOfMovedValue,
    /// Cannot assign to immutable borrow
    CannotAssignToImmutable,
    /// Lifetime constraint not satisfied
    ConstraintNotSatisfied,
}

// ============================================================================
// Lifetime Analyzer
// ============================================================================

/// Analyzes lifetimes and borrow checking
pub struct LifetimeAnalyzer {
    /// Lifetime constraints collected during analysis
    constraints: Vec<LifetimeConstraint>,
    /// Lifetime substitutions (reserved for lifetime inference)
    #[allow(dead_code)]
    substitutions: HashMap<LifetimeId, Lifetime>,
    /// Borrow state for current scope
    borrow_state: BorrowState,
    /// Collected errors
    errors: Vec<LifetimeError>,
    /// Lifetime ID counter
    next_lifetime_id: usize,
}

impl LifetimeAnalyzer {
    /// Create a new lifetime analyzer
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            substitutions: HashMap::new(),
            borrow_state: BorrowState::new(),
            errors: Vec::new(),
            next_lifetime_id: 0,
        }
    }

    /// Create a fresh lifetime
    pub fn fresh_lifetime(&mut self) -> LifetimeId {
        let id = LifetimeId(self.next_lifetime_id);
        self.next_lifetime_id += 1;
        id
    }

    /// Add a constraint that `longer` outlives `shorter`
    pub fn add_outlives_constraint(
        &mut self,
        longer: LifetimeId,
        shorter: LifetimeId,
        span: Option<(usize, usize)>,
    ) {
        self.constraints.push(LifetimeConstraint {
            longer,
            shorter,
            span,
        });
    }

    /// Record a borrow
    pub fn borrow(
        &mut self,
        path: String,
        lifetime: LifetimeId,
        is_mutable: bool,
        span: (usize, usize),
    ) -> Option<BorrowId> {
        match self.borrow_state.borrow(path, lifetime, is_mutable, span) {
            Ok(id) => Some(id),
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    /// End a borrow
    pub fn end_borrow(&mut self, id: BorrowId) {
        self.borrow_state.end_borrow(id);
    }

    /// Record a move
    pub fn move_value(&mut self, path: &str, span: (usize, usize)) {
        if let Err(error) = self.borrow_state.move_value(path, span) {
            self.errors.push(error);
        }
    }

    /// Check a use of a value
    pub fn check_use(&mut self, path: &str, span: (usize, usize)) {
        if !self.borrow_state.is_valid(path) {
            self.errors.push(LifetimeError {
                message: format!("Use of moved value: `{}`", path),
                kind: LifetimeErrorKind::UseAfterMove,
                span: Some(span),
                related_spans: vec![],
            });
        }
    }

    /// Check assignment to a borrowed value
    pub fn check_assignment(&mut self, path: &str, span: (usize, usize)) {
        if self.borrow_state.has_any_borrow(path) {
            self.errors.push(LifetimeError {
                message: format!("Cannot assign to `{}` while borrowed", path),
                kind: LifetimeErrorKind::CannotAssignToImmutable,
                span: Some(span),
                related_spans: vec![],
            });
        }
    }

    /// Infer lifetime for a reference type
    pub fn infer_reference_lifetime(&mut self, ty: &RustType) -> Option<LifetimeId> {
        match ty {
            RustType::Reference { lifetime, .. } => {
                if let Some(Lifetime::Named { id, .. } | Lifetime::Inferred(id)) = lifetime {
                    Some(*id)
                } else if let Some(Lifetime::Static) = lifetime {
                    // 'static has a special ID
                    Some(LifetimeId(usize::MAX))
                } else {
                    // Create a fresh lifetime for anonymous/none
                    Some(self.fresh_lifetime())
                }
            }
            _ => None,
        }
    }

    /// Check if a type contains any lifetime that needs checking
    pub fn type_has_lifetime(&self, ty: &RustType) -> bool {
        match ty {
            RustType::Reference { .. } => true,
            RustType::Named { type_args, .. } => {
                type_args.iter().any(|t| self.type_has_lifetime(t))
            }
            RustType::Tuple(elements) => elements.iter().any(|t| self.type_has_lifetime(t)),
            RustType::Array { element, .. } | RustType::Slice(element) => {
                self.type_has_lifetime(element)
            }
            _ => false,
        }
    }

    /// Validate all collected constraints
    pub fn validate_constraints(&mut self) -> bool {
        // Build a graph of lifetime relationships
        let mut outlives: HashMap<LifetimeId, HashSet<LifetimeId>> = HashMap::new();

        for constraint in &self.constraints {
            outlives
                .entry(constraint.longer)
                .or_default()
                .insert(constraint.shorter);
        }

        // Check for cycles (which would indicate unsatisfiable constraints)
        for constraint in &self.constraints {
            if self.has_cycle(&outlives, constraint.shorter, constraint.longer) {
                self.errors.push(LifetimeError {
                    message: "Lifetime constraint cycle detected".to_string(),
                    kind: LifetimeErrorKind::ConstraintNotSatisfied,
                    span: constraint.span,
                    related_spans: vec![],
                });
                return false;
            }
        }

        true
    }

    /// Check if there's a cycle in lifetime constraints
    fn has_cycle(
        &self,
        outlives: &HashMap<LifetimeId, HashSet<LifetimeId>>,
        from: LifetimeId,
        to: LifetimeId,
    ) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![from];

        while let Some(current) = stack.pop() {
            if current == to {
                return true;
            }

            if visited.insert(current) {
                if let Some(children) = outlives.get(&current) {
                    stack.extend(children.iter().copied());
                }
            }
        }

        false
    }

    /// Get collected errors
    pub fn errors(&self) -> &[LifetimeError] {
        &self.errors
    }

    /// Check if analysis has errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Clear errors
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) -> BorrowState {
        let child = self.borrow_state.child();
        std::mem::replace(&mut self.borrow_state, child)
    }

    /// Exit a scope, restoring previous state
    pub fn exit_scope(&mut self, previous: BorrowState) {
        self.borrow_state = previous;
    }
}

impl Default for LifetimeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
