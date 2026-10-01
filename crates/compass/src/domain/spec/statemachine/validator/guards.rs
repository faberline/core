//! Guard reference checks.

use super::{Severity, StateMachineValidator, ValidationError, ValidationResult};
use crate::domain::spec::statemachine::schema::{StateMachineDef, StateNodeDef, TransitionInput};
use std::collections::{HashMap, HashSet};

impl StateMachineValidator {
    /// Validate guard references with location tracking
    pub(super) fn validate_guards(&self, machine: &StateMachineDef, result: &mut ValidationResult) {
        let defined: HashSet<_> = machine.guards.keys().cloned().collect();
        let used = self.collect_used_guards_with_locations(&machine.states, "states");

        // Sort guards for deterministic iteration order
        let mut guards: Vec<_> = used.into_iter().collect();
        guards.sort_by(|a, b| a.0.cmp(&b.0));

        for (guard, mut locations) in guards {
            if !defined.contains(&guard) {
                // Sort locations for deterministic output
                locations.sort();
                // Emit one warning per location for complete reporting
                for location in locations {
                    *result = std::mem::take(result).with_warning(ValidationError {
                        code: "UNDEFINED_GUARD".into(),
                        message: format!("Guard '{}' used but not defined", guard),
                        path: location,
                        severity: Severity::Warning,
                    });
                }
            }
        }
    }

    /// Collect guards with their usage locations
    fn collect_used_guards_with_locations(
        &self,
        states: &HashMap<String, StateNodeDef>,
        base_path: &str,
    ) -> HashMap<String, Vec<String>> {
        let mut guards: HashMap<String, Vec<String>> = HashMap::new();

        for (state_id, node) in states {
            let state_path = format!("{}.{}", base_path, state_id);

            if let Some(ref on) = node.on {
                for (event, transition) in on {
                    let event_path = format!("{}.on.{}", state_path, event);
                    self.collect_guards_from_transition_with_location(
                        transition,
                        &event_path,
                        &mut guards,
                    );
                }
            }

            if let Some(ref substates) = node.states {
                let nested = self.collect_used_guards_with_locations(
                    substates,
                    &format!("{}.states", state_path),
                );
                for (guard, locs) in nested {
                    guards.entry(guard).or_default().extend(locs);
                }
            }
        }
        guards
    }

    fn collect_guards_from_transition_with_location(
        &self,
        transition: &TransitionInput,
        path: &str,
        guards: &mut HashMap<String, Vec<String>>,
    ) {
        match transition {
            TransitionInput::Simple(_) => {}
            TransitionInput::Detailed(detail) => {
                if let Some(ref guard) = detail.guard {
                    guards
                        .entry(guard.clone())
                        .or_default()
                        .push(path.to_string());
                }
            }
            TransitionInput::Conditional(conditions) => {
                for (i, detail) in conditions.iter().enumerate() {
                    if let Some(ref guard) = detail.guard {
                        guards
                            .entry(guard.clone())
                            .or_default()
                            .push(format!("{}[{}]", path, i));
                    }
                }
            }
        }
    }
}
