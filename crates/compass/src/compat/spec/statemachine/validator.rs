//! State machine semantic validator
//!
//! Validates state machine definitions for:
//! - Structural correctness (initial exists, targets valid)
//! - Semantic correctness (reachability, guard/action references)
//!
//! Uses path-qualified IDs (e.g., "parent.child") internally to handle
//! nested states correctly and avoid ID collisions.

pub use crate::domain::spec::statemachine::validator::{
    Severity, StateMachineValidator, ValidationError, ValidationResult,
};
