//! stateDiagram parsing.

use super::{MermaidError, MermaidParser};
use crate::domain::spec::ir::{StateDef, StateMachineSpec, TransitionDef};

impl MermaidParser {
    /// Parse stateDiagram
    pub(super) fn parse_state_diagram(
        &self,
        content: &str,
    ) -> Result<StateMachineSpec, MermaidError> {
        let mut spec = StateMachineSpec::default();

        for line in content.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            // State definition: state "State Name" as alias
            if line.starts_with("state ") {
                if let Some(state) = self.parse_state_def(line)? {
                    spec.states.push(state);
                }
            }
            // Transition: State1 --> State2 : event
            else if line.contains("-->") {
                if let Some(transition) = self.parse_transition(line)? {
                    // Check for initial state
                    if transition.from == "[*]" {
                        spec.initial_state = Some(transition.to.clone());
                    } else if transition.to == "[*]" {
                        spec.final_states.push(transition.from.clone());
                    }
                    spec.transitions.push(transition);
                }
            }
        }

        Ok(spec)
    }

    /// Parse state definition
    fn parse_state_def(&self, line: &str) -> Result<Option<StateDef>, MermaidError> {
        // state "Description" as StateName
        let without_state = line.strip_prefix("state ").unwrap_or(line).trim();

        let (description, name) = if without_state.starts_with('"') {
            // Has description
            if let Some(end_quote) = without_state[1..].find('"') {
                let desc = &without_state[1..end_quote + 1];
                let rest = without_state[end_quote + 2..].trim();
                let name = rest.strip_prefix("as ").unwrap_or(rest).trim();
                (Some(desc.to_string()), name.to_string())
            } else {
                (None, without_state.to_string())
            }
        } else {
            (
                None,
                without_state
                    .split_whitespace()
                    .next()
                    .unwrap_or(without_state)
                    .to_string(),
            )
        };

        Ok(Some(StateDef {
            name,
            description,
            on_enter: None,
            on_exit: None,
            nested: None,
        }))
    }

    /// Parse transition
    fn parse_transition(&self, line: &str) -> Result<Option<TransitionDef>, MermaidError> {
        let parts: Vec<&str> = line.split("-->").collect();
        if parts.len() < 2 {
            return Ok(None);
        }

        let from = parts[0].trim().to_string();
        let to_and_event = parts[1].trim();

        let (to, event) = if let Some(colon) = to_and_event.find(':') {
            (
                to_and_event[..colon].trim().to_string(),
                Some(to_and_event[colon + 1..].trim().to_string()),
            )
        } else {
            (to_and_event.to_string(), None)
        };

        Ok(Some(TransitionDef {
            from,
            to,
            event,
            guard: None,
            action: None,
        }))
    }
}
