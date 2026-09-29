//! State machine semantic validator
//!
//! Validates state machine definitions for:
//! - Structural correctness (initial exists, targets valid)
//! - Semantic correctness (reachability, guard/action references)
//!
//! Uses path-qualified IDs (e.g., "parent.child") internally to handle
//! nested states correctly and avoid ID collisions.

use super::schema::{StateMachineDef, StateNodeDef};
use serde::Serialize;

mod actions;
mod guards;
mod reachability;
mod registry;
mod states;

/// Validation result
#[derive(Debug, Clone, Serialize, Default)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn with_error(mut self, error: ValidationError) -> Self {
        self.valid = false;
        self.errors.push(error);
        self
    }

    pub fn with_warning(mut self, warning: ValidationError) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Validation error/warning
#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub path: String,
    pub severity: Severity,
}

/// Error severity
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// State info for validation - includes node reference and path
#[derive(Debug, Clone)]
struct StateInfo<'a> {
    /// The state node definition
    node: &'a StateNodeDef,
    /// Path-qualified ID (e.g., "parent.child")
    path_id: String,
    /// Simple ID (last component)
    simple_id: String,
}

/// State machine validator
pub struct StateMachineValidator {
    /// Strict mode: treats warnings as errors for certain checks
    strict: bool,
}

impl StateMachineValidator {
    pub fn new() -> Self {
        Self { strict: false }
    }

    /// Create a validator with strict mode enabled
    /// In strict mode, MISSING_COMPOUND_INITIAL becomes an error
    pub fn strict() -> Self {
        Self { strict: true }
    }

    /// Enable or disable strict mode
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Validate a state machine definition
    pub fn validate(&self, machine: &StateMachineDef) -> ValidationResult {
        let mut result = ValidationResult::ok();

        // Build full state registry with path-qualified IDs
        let state_registry = self.build_state_registry(&machine.states, "");

        // Build simple ID to path-qualified ID mapping for target resolution
        let simple_to_paths = self.build_simple_id_map(&state_registry);

        // 1. Check initial state exists
        if !machine.states.contains_key(&machine.initial) {
            result = result.with_error(ValidationError {
                code: "MISSING_INITIAL_STATE".into(),
                message: format!("Initial state '{}' not found in states", machine.initial),
                path: "initial".into(),
                severity: Severity::Error,
            });
        }

        // 2. Check for ambiguous state IDs (same simple ID in multiple places)
        for (simple_id, paths) in &simple_to_paths {
            if paths.len() > 1 {
                result = result.with_warning(ValidationError {
                    code: "AMBIGUOUS_STATE_ID".into(),
                    message: format!(
                        "State ID '{}' appears in multiple locations: {}. Use path-qualified IDs to avoid ambiguity.",
                        simple_id,
                        paths.join(", ")
                    ),
                    path: "states".into(),
                    severity: Severity::Warning,
                });
            }
        }

        // 3. Validate all states and transitions
        self.validate_states(
            &machine.states,
            &state_registry,
            &simple_to_paths,
            "states",
            &mut result,
        );

        // 4. Check for unreachable states (only top-level, nested handled separately)
        let reachable = self.find_reachable_states(machine, &state_registry, &simple_to_paths);
        for state_id in machine.states.keys() {
            if !reachable.contains(state_id) && state_id != &machine.initial {
                result = result.with_warning(ValidationError {
                    code: "UNREACHABLE_STATE".into(),
                    message: format!("State '{}' is not reachable from initial state", state_id),
                    path: format!("states.{}", state_id),
                    severity: Severity::Warning,
                });
            }
        }

        // 5. Validate guard references
        self.validate_guards(machine, &mut result);

        // 6. Validate action references
        self.validate_actions(machine, &mut result);

        result
    }
}

impl Default for StateMachineValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
