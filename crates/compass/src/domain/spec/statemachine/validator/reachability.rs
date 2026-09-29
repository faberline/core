//! Reachability analysis.

use super::{StateInfo, StateMachineValidator};
use crate::domain::spec::statemachine::schema::{StateMachineDef, StateType, TransitionInput};
use std::collections::{HashMap, HashSet};

impl StateMachineValidator {
    /// Find all reachable states from initial
    /// Uses the full registry to correctly traverse nested state transitions
    pub(super) fn find_reachable_states(
        &self,
        machine: &StateMachineDef,
        registry: &HashMap<String, StateInfo>,
        simple_to_paths: &HashMap<String, Vec<String>>,
    ) -> HashSet<String> {
        let mut reachable = HashSet::new();
        // Start with the initial state (using its simple ID)
        let mut queue = vec![machine.initial.clone()];

        while let Some(state_id) = queue.pop() {
            if reachable.contains(&state_id) {
                continue;
            }
            reachable.insert(state_id.clone());

            // Find the state info - could be a simple ID or path-qualified
            let state_info = registry.get(&state_id).or_else(|| {
                // Try to resolve simple ID
                simple_to_paths
                    .get(&state_id)
                    .and_then(|paths| paths.first())
                    .and_then(|path| registry.get(path))
            });

            if let Some(info) = state_info {
                // Add transitions - traverse the node's transitions
                if let Some(ref on) = info.node.on {
                    for transition in on.values() {
                        for target in self.get_transition_targets(transition) {
                            // Resolve the target to handle both simple and path-qualified IDs
                            if let Some(resolved) = self.resolve_target(
                                &target,
                                &info.path_id,
                                registry,
                                simple_to_paths,
                            ) {
                                // Add the simple ID (for top-level compatibility) and path ID
                                if !reachable.contains(&resolved) {
                                    queue.push(resolved.clone());
                                }
                                // Also add the simple ID if it's different
                                if let Some(resolved_info) = registry.get(&resolved) {
                                    if !reachable.contains(&resolved_info.simple_id) {
                                        queue.push(resolved_info.simple_id.clone());
                                    }
                                }
                            } else if !reachable.contains(&target) {
                                // Use the target as-is (might be unresolved)
                                queue.push(target);
                            }
                        }
                    }
                }

                // For compound states, only the initial substate is automatically reachable
                // (not ALL children - that was the bug)
                if let Some(ref substates) = info.node.states {
                    if let Some(ref initial) = info.node.initial {
                        // Only the initial substate is automatically reachable
                        let initial_path = format!("{}.{}", info.path_id, initial);
                        if !reachable.contains(&initial_path) {
                            queue.push(initial_path);
                        }
                        if !reachable.contains(initial) {
                            queue.push(initial.clone());
                        }
                    } else {
                        // Parallel state or no initial - all children reachable
                        let node_type = info.node.node_type.as_ref().unwrap_or(&StateType::Atomic);
                        if *node_type == StateType::Parallel {
                            for substate_id in substates.keys() {
                                let substate_path = format!("{}.{}", info.path_id, substate_id);
                                if !reachable.contains(&substate_path) {
                                    queue.push(substate_path);
                                }
                                if !reachable.contains(substate_id) {
                                    queue.push(substate_id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        reachable
    }

    /// Get all targets from a transition
    fn get_transition_targets(&self, transition: &TransitionInput) -> Vec<String> {
        match transition {
            TransitionInput::Simple(target) => vec![target.clone()],
            TransitionInput::Detailed(detail) => detail.target.clone().into_iter().collect(),
            TransitionInput::Conditional(conditions) => {
                conditions.iter().filter_map(|d| d.target.clone()).collect()
            }
        }
    }
}
