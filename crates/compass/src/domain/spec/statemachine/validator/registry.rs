//! State registry: path-qualified IDs and target resolution.

use super::{StateInfo, StateMachineValidator};
use crate::domain::spec::statemachine::schema::StateNodeDef;
use std::collections::HashMap;

impl StateMachineValidator {
    /// Build a registry of all states with path-qualified IDs
    pub(super) fn build_state_registry<'a>(
        &self,
        states: &'a HashMap<String, StateNodeDef>,
        parent_path: &str,
    ) -> HashMap<String, StateInfo<'a>> {
        let mut registry = HashMap::new();

        for (id, node) in states {
            let path_id = if parent_path.is_empty() {
                id.clone()
            } else {
                format!("{}.{}", parent_path, id)
            };

            registry.insert(
                path_id.clone(),
                StateInfo {
                    node,
                    path_id: path_id.clone(),
                    simple_id: id.clone(),
                },
            );

            // Recurse into nested states
            if let Some(ref substates) = node.states {
                let nested = self.build_state_registry(substates, &path_id);
                registry.extend(nested);
            }
        }

        registry
    }

    /// Build mapping from simple IDs to all path-qualified IDs
    pub(super) fn build_simple_id_map(
        &self,
        registry: &HashMap<String, StateInfo>,
    ) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        for info in registry.values() {
            map.entry(info.simple_id.clone())
                .or_default()
                .push(info.path_id.clone());
        }

        map
    }

    /// Resolve a transition target to a path-qualified ID
    /// Supports both simple IDs and path-qualified IDs
    pub(super) fn resolve_target(
        &self,
        target: &str,
        current_path: &str,
        registry: &HashMap<String, StateInfo>,
        simple_to_paths: &HashMap<String, Vec<String>>,
    ) -> Option<String> {
        // First, try exact match (path-qualified ID)
        if registry.contains_key(target) {
            return Some(target.to_string());
        }

        // Try as simple ID - look for matches
        if let Some(paths) = simple_to_paths.get(target) {
            if paths.len() == 1 {
                // Unambiguous - return the only match
                return Some(paths[0].clone());
            } else if paths.len() > 1 {
                // Ambiguous - try to find sibling or ancestor match
                // Prefer sibling (same parent) first
                let current_parent = if current_path.contains('.') {
                    current_path.rsplit_once('.').map(|(p, _)| p).unwrap_or("")
                } else {
                    ""
                };

                // Look for sibling
                let sibling_path = if current_parent.is_empty() {
                    target.to_string()
                } else {
                    format!("{}.{}", current_parent, target)
                };

                if paths.contains(&sibling_path) {
                    return Some(sibling_path);
                }

                // Return first match (ambiguous, but valid)
                return Some(paths[0].clone());
            }
        }

        None
    }
}
