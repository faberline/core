//! Action reference checks.

use super::{Severity, StateMachineValidator, ValidationError, ValidationResult};
use crate::domain::spec::statemachine::schema::{StateMachineDef, StateNodeDef, TransitionInput};
use std::collections::{HashMap, HashSet};

impl StateMachineValidator {
    /// Validate action references with location tracking
    pub(super) fn validate_actions(
        &self,
        machine: &StateMachineDef,
        result: &mut ValidationResult,
    ) {
        let defined: HashSet<_> = machine.actions.keys().cloned().collect();
        let used = self.collect_used_actions_with_locations(&machine.states, "states");

        // Sort actions for deterministic iteration order
        let mut actions: Vec<_> = used.into_iter().collect();
        actions.sort_by(|a, b| a.0.cmp(&b.0));

        for (action, mut locations) in actions {
            if !defined.contains(&action) {
                // Sort locations for deterministic output
                locations.sort();
                // Emit one warning per location for complete reporting
                for location in locations {
                    *result = std::mem::take(result).with_warning(ValidationError {
                        code: "UNDEFINED_ACTION".into(),
                        message: format!("Action '{}' used but not defined", action),
                        path: location,
                        severity: Severity::Warning,
                    });
                }
            }
        }
    }

    /// Collect actions with their usage locations
    fn collect_used_actions_with_locations(
        &self,
        states: &HashMap<String, StateNodeDef>,
        base_path: &str,
    ) -> HashMap<String, Vec<String>> {
        let mut actions: HashMap<String, Vec<String>> = HashMap::new();

        for (state_id, node) in states {
            let state_path = format!("{}.{}", base_path, state_id);

            // Entry actions
            if let Some(ref entry) = node.entry {
                for action in entry.to_vec() {
                    actions
                        .entry(action)
                        .or_default()
                        .push(format!("{}.entry", state_path));
                }
            }

            // Exit actions
            if let Some(ref exit) = node.exit {
                for action in exit.to_vec() {
                    actions
                        .entry(action)
                        .or_default()
                        .push(format!("{}.exit", state_path));
                }
            }

            // Transition actions
            if let Some(ref on) = node.on {
                for (event, transition) in on {
                    let event_path = format!("{}.on.{}", state_path, event);
                    self.collect_actions_from_transition_with_location(
                        transition,
                        &event_path,
                        &mut actions,
                    );
                }
            }

            if let Some(ref substates) = node.states {
                let nested = self.collect_used_actions_with_locations(
                    substates,
                    &format!("{}.states", state_path),
                );
                for (action, locs) in nested {
                    actions.entry(action).or_default().extend(locs);
                }
            }
        }
        actions
    }

    fn collect_actions_from_transition_with_location(
        &self,
        transition: &TransitionInput,
        path: &str,
        actions: &mut HashMap<String, Vec<String>>,
    ) {
        match transition {
            TransitionInput::Simple(_) => {}
            TransitionInput::Detailed(detail) => {
                if let Some(ref action_ref) = detail.actions {
                    for action in action_ref.to_vec() {
                        actions.entry(action).or_default().push(path.to_string());
                    }
                }
            }
            TransitionInput::Conditional(conditions) => {
                for (i, detail) in conditions.iter().enumerate() {
                    if let Some(ref action_ref) = detail.actions {
                        for action in action_ref.to_vec() {
                            actions
                                .entry(action)
                                .or_default()
                                .push(format!("{}[{}]", path, i));
                        }
                    }
                }
            }
        }
    }
}
