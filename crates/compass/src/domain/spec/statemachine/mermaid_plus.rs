//! Mermaid+ generator
//!
//! Generates Mermaid+ output from validated state machine definitions.
//! Mermaid+ = YAML frontmatter (structured definition) + Mermaid diagram

use super::schema::{
    ActionRef, StateMachineDef, StateNodeDef, StateType, TransitionDetail, TransitionInput,
};
use super::validator::ValidationResult;
use serde::Serialize;
use std::collections::HashMap;

/// Mermaid+ output structure
#[derive(Debug, Clone, Serialize)]
pub struct MermaidPlusOutput {
    /// YAML frontmatter content (without --- markers)
    pub frontmatter: String,
    /// Mermaid diagram content (without ```mermaid``` markers)
    pub diagram: String,
    /// Validation result
    pub validation: ValidationResult,
    /// Combined Mermaid+ format (ready to embed in markdown)
    pub combined: String,
}

/// Mermaid+ generator
pub struct MermaidPlusGenerator;

impl MermaidPlusGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate Mermaid+ output from a state machine definition
    pub fn generate(
        &self,
        machine: &StateMachineDef,
        validation: ValidationResult,
    ) -> Result<MermaidPlusOutput, String> {
        // Generate YAML frontmatter
        let frontmatter = self.generate_frontmatter(machine)?;

        // Generate Mermaid diagram
        let diagram = self.generate_mermaid(machine)?;

        // Combine into Mermaid+ format (frontmatter inside code block per Mermaid spec)
        let mut combined = String::new();
        combined.push_str("```mermaid\n");
        combined.push_str("---\n");
        combined.push_str(&frontmatter);
        combined.push_str("---\n");
        combined.push_str(&diagram);
        combined.push_str("```\n");

        // Add validation warnings as HTML comments
        if !validation.warnings.is_empty() {
            combined.push_str("\n<!-- Validation Warnings:\n");
            for w in &validation.warnings {
                combined.push_str(&format!("  - {}: {} (at {})\n", w.code, w.message, w.path));
            }
            combined.push_str("-->\n");
        }

        Ok(MermaidPlusOutput {
            frontmatter,
            diagram,
            validation,
            combined,
        })
    }

    /// Generate YAML frontmatter from machine definition
    fn generate_frontmatter(&self, machine: &StateMachineDef) -> Result<String, String> {
        // Use serde_yaml but strip the leading "---\n" if present
        let yaml = serde_yaml::to_string(machine)
            .map_err(|e| format!("YAML serialization error: {}", e))?;

        // serde_yaml adds "---\n" at the start, strip it since we add our own
        let yaml = yaml.strip_prefix("---\n").unwrap_or(&yaml);

        Ok(yaml.to_string())
    }

    /// Generate Mermaid stateDiagram-v2 from machine definition
    fn generate_mermaid(&self, machine: &StateMachineDef) -> Result<String, String> {
        let mut mermaid = String::new();
        mermaid.push_str("stateDiagram-v2\n");

        // Add initial transition
        mermaid.push_str(&format!("    [*] --> {}\n", machine.initial));

        // Generate states and transitions
        self.generate_states(&machine.states, &mut mermaid, "    ")?;

        Ok(mermaid)
    }

    /// Generate Mermaid for states recursively
    fn generate_states(
        &self,
        states: &HashMap<String, StateNodeDef>,
        mermaid: &mut String,
        indent: &str,
    ) -> Result<(), String> {
        // Sort states for consistent output
        let mut state_ids: Vec<_> = states.keys().collect();
        state_ids.sort();

        for state_id in state_ids {
            let node = &states[state_id];
            let node_type = node.node_type.as_ref().unwrap_or(&StateType::Atomic);

            // Handle compound/parallel states
            if let Some(ref substates) = node.states {
                if *node_type == StateType::Parallel {
                    // Parallel state with region separators
                    if let Some(ref desc) = node.description {
                        mermaid
                            .push_str(&format!("{}state \"{}\" as {}\n", indent, desc, state_id));
                    }
                    mermaid.push_str(&format!("{}state {} {{\n", indent, state_id));

                    // Render each child as a separate region with -- separators
                    let mut substate_ids: Vec<_> = substates.keys().collect();
                    substate_ids.sort();

                    let child_indent = format!("{}    ", indent);
                    for (i, substate_id) in substate_ids.iter().enumerate() {
                        if i > 0 {
                            // Add region separator between parallel regions
                            mermaid.push_str(&format!("{}--\n", child_indent));
                        }
                        // Generate the substate inline (not recursive for parallel regions)
                        let subnode = &substates[*substate_id];
                        self.generate_single_state(substate_id, subnode, mermaid, &child_indent)?;
                    }
                } else {
                    // Compound state
                    if let Some(ref desc) = node.description {
                        mermaid
                            .push_str(&format!("{}state \"{}\" as {}\n", indent, desc, state_id));
                    }
                    mermaid.push_str(&format!("{}state {} {{\n", indent, state_id));

                    // Add initial for compound
                    if let Some(ref initial) = node.initial {
                        mermaid.push_str(&format!("{}    [*] --> {}\n", indent, initial));
                    }

                    self.generate_states(substates, mermaid, &format!("{}    ", indent))?;
                }
                mermaid.push_str(&format!("{}}}\n", indent));
            } else if *node_type == StateType::Final {
                // Final state - add transition to [*]
                mermaid.push_str(&format!("{}{} --> [*]\n", indent, state_id));
            } else {
                // Regular state with description
                if let Some(ref desc) = node.description {
                    mermaid.push_str(&format!("{}state \"{}\" as {}\n", indent, desc, state_id));
                }
            }

            // Generate transitions
            if let Some(ref on) = node.on {
                let mut events: Vec<_> = on.keys().collect();
                events.sort();

                for event in events {
                    let transition = &on[event];
                    self.generate_transition(state_id, event, transition, mermaid, indent)?;
                }
            }
        }

        Ok(())
    }

    /// Generate Mermaid for a single state (used for parallel regions)
    fn generate_single_state(
        &self,
        state_id: &str,
        node: &StateNodeDef,
        mermaid: &mut String,
        indent: &str,
    ) -> Result<(), String> {
        let node_type = node.node_type.as_ref().unwrap_or(&StateType::Atomic);

        // Handle nested compound/parallel states within parallel regions
        if let Some(ref substates) = node.states {
            if let Some(ref desc) = node.description {
                mermaid.push_str(&format!("{}state \"{}\" as {}\n", indent, desc, state_id));
            }
            mermaid.push_str(&format!("{}state {} {{\n", indent, state_id));

            // Check if this is a nested parallel state
            if *node_type == StateType::Parallel {
                // Render nested parallel with region separators
                let mut substate_ids: Vec<_> = substates.keys().collect();
                substate_ids.sort();

                let child_indent = format!("{}    ", indent);
                for (i, substate_id) in substate_ids.iter().enumerate() {
                    if i > 0 {
                        mermaid.push_str(&format!("{}--\n", child_indent));
                    }
                    let subnode = &substates[*substate_id];
                    self.generate_single_state(substate_id, subnode, mermaid, &child_indent)?;
                }
            } else {
                // Compound state - add initial and recurse
                if let Some(ref initial) = node.initial {
                    mermaid.push_str(&format!("{}    [*] --> {}\n", indent, initial));
                }
                self.generate_states(substates, mermaid, &format!("{}    ", indent))?;
            }

            mermaid.push_str(&format!("{}}}\n", indent));
        } else if *node_type == StateType::Final {
            mermaid.push_str(&format!("{}{} --> [*]\n", indent, state_id));
        } else {
            // Regular atomic state - always emit state declaration for visibility in parallel regions
            if let Some(ref desc) = node.description {
                mermaid.push_str(&format!("{}state \"{}\" as {}\n", indent, desc, state_id));
            } else {
                // Emit simple state declaration to ensure visibility
                mermaid.push_str(&format!("{}state {}\n", indent, state_id));
            }
        }

        // Generate transitions
        if let Some(ref on) = node.on {
            let mut events: Vec<_> = on.keys().collect();
            events.sort();

            for event in events {
                let transition = &on[event];
                self.generate_transition(state_id, event, transition, mermaid, indent)?;
            }
        }

        Ok(())
    }

    /// Generate Mermaid for a transition
    fn generate_transition(
        &self,
        from: &str,
        event: &str,
        transition: &TransitionInput,
        mermaid: &mut String,
        indent: &str,
    ) -> Result<(), String> {
        match transition {
            TransitionInput::Simple(target) => {
                mermaid.push_str(&format!("{}{} --> {}: {}\n", indent, from, target, event));
            }
            TransitionInput::Detailed(detail) => {
                self.generate_detailed_transition(from, event, detail, mermaid, indent)?;
            }
            TransitionInput::Conditional(conditions) => {
                for detail in conditions {
                    self.generate_detailed_transition(from, event, detail, mermaid, indent)?;
                }
            }
        }
        Ok(())
    }

    fn generate_detailed_transition(
        &self,
        from: &str,
        event: &str,
        detail: &TransitionDetail,
        mermaid: &mut String,
        indent: &str,
    ) -> Result<(), String> {
        // Determine target: use explicit target or self (internal transition)
        let target = detail.target.as_deref().unwrap_or(from);

        let mut label = event.to_string();

        // Add guard
        if let Some(ref guard) = detail.guard {
            label = format!("{} [{}]", label, guard);
        }

        // Add actions
        if let Some(ref actions) = detail.actions {
            let action_str = match actions {
                ActionRef::Single(a) => a.clone(),
                ActionRef::Multiple(list) => list.join(", "),
            };
            label = format!("{} / {}", label, action_str);
        }

        mermaid.push_str(&format!("{}{} --> {}: {}\n", indent, from, target, label));
        Ok(())
    }
}

impl Default for MermaidPlusGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
