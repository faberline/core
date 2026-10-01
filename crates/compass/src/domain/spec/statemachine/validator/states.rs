//! State and transition checks.

use super::{Severity, StateInfo, StateMachineValidator, ValidationError, ValidationResult};
use crate::domain::spec::statemachine::schema::{
    StateNodeDef, StateType, TransitionDetail, TransitionInput,
};
use std::collections::HashMap;

impl StateMachineValidator {
    /// Validate states and their transitions
    pub(super) fn validate_states(
        &self,
        states: &HashMap<String, StateNodeDef>,
        registry: &HashMap<String, StateInfo>,
        simple_to_paths: &HashMap<String, Vec<String>>,
        path: &str,
        result: &mut ValidationResult,
    ) {
        for (state_id, node) in states {
            let state_path = format!("{}.{}", path, state_id);
            // Calculate path-qualified ID for this state
            let path_id = state_path
                .strip_prefix("states.")
                .unwrap_or(&state_path)
                .to_string();

            // Check compound state has initial
            if node.states.is_some() && node.initial.is_none() {
                let node_type = node.node_type.as_ref().unwrap_or(&StateType::Compound);
                if *node_type != StateType::Parallel {
                    let error = ValidationError {
                        code: "MISSING_COMPOUND_INITIAL".into(),
                        message: format!(
                            "Compound state '{}' should have an initial substate",
                            state_id
                        ),
                        path: state_path.clone(),
                        severity: if self.strict {
                            Severity::Error
                        } else {
                            Severity::Warning
                        },
                    };
                    if self.strict {
                        *result = std::mem::take(result).with_error(error);
                    } else {
                        *result = std::mem::take(result).with_warning(error);
                    }
                }
            }

            // Validate compound initial exists in substates
            if let (Some(ref initial), Some(ref substates)) = (&node.initial, &node.states) {
                if !substates.contains_key(initial) {
                    *result = std::mem::take(result).with_error(ValidationError {
                        code: "INVALID_COMPOUND_INITIAL".into(),
                        message: format!(
                            "Compound state '{}' initial '{}' not found in substates",
                            state_id, initial
                        ),
                        path: format!("{}.initial", state_path),
                        severity: Severity::Error,
                    });
                }
            }

            // Validate transitions
            if let Some(ref on) = node.on {
                for (event, transition) in on {
                    self.validate_transition(
                        transition,
                        registry,
                        simple_to_paths,
                        &path_id,
                        &format!("{}.on.{}", state_path, event),
                        result,
                    );
                }
            }

            // Recurse into nested states
            if let Some(ref substates) = node.states {
                self.validate_states(
                    substates,
                    registry,
                    simple_to_paths,
                    &format!("{}.states", state_path),
                    result,
                );
            }
        }
    }

    /// Validate transition targets
    fn validate_transition(
        &self,
        transition: &TransitionInput,
        registry: &HashMap<String, StateInfo>,
        simple_to_paths: &HashMap<String, Vec<String>>,
        current_path: &str,
        error_path: &str,
        result: &mut ValidationResult,
    ) {
        match transition {
            TransitionInput::Simple(target) => {
                if self
                    .resolve_target(target, current_path, registry, simple_to_paths)
                    .is_none()
                {
                    *result = std::mem::take(result).with_error(ValidationError {
                        code: "INVALID_TRANSITION_TARGET".into(),
                        message: format!("Transition target '{}' not found", target),
                        path: error_path.into(),
                        severity: Severity::Error,
                    });
                }
            }
            TransitionInput::Detailed(detail) => {
                self.validate_transition_detail(
                    detail,
                    registry,
                    simple_to_paths,
                    current_path,
                    error_path,
                    result,
                );
            }
            TransitionInput::Conditional(conditions) => {
                for (i, detail) in conditions.iter().enumerate() {
                    self.validate_transition_detail(
                        detail,
                        registry,
                        simple_to_paths,
                        current_path,
                        &format!("{}[{}]", error_path, i),
                        result,
                    );
                }
            }
        }
    }

    fn validate_transition_detail(
        &self,
        detail: &TransitionDetail,
        registry: &HashMap<String, StateInfo>,
        simple_to_paths: &HashMap<String, Vec<String>>,
        current_path: &str,
        error_path: &str,
        result: &mut ValidationResult,
    ) {
        if let Some(ref target) = detail.target {
            if self
                .resolve_target(target, current_path, registry, simple_to_paths)
                .is_none()
            {
                *result = std::mem::take(result).with_error(ValidationError {
                    code: "INVALID_TRANSITION_TARGET".into(),
                    message: format!("Transition target '{}' not found", target),
                    path: error_path.into(),
                    severity: Severity::Error,
                });
            }
        }
    }
}
