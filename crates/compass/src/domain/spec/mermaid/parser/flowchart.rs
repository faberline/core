//! flowchart parsing.

use super::{MermaidError, MermaidParser};
use crate::domain::spec::ir::{ControlFlowSpec, FlowEdge, FlowNode, FlowNodeType};

impl MermaidParser {
    /// Parse flowchart
    pub(super) fn parse_flowchart(&self, content: &str) -> Result<ControlFlowSpec, MermaidError> {
        let mut spec = ControlFlowSpec::default();

        for line in content.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            // Parse node definitions and edges
            if line.contains("-->") || line.contains("---") {
                self.parse_flow_line(line, &mut spec)?;
            }
        }

        Ok(spec)
    }

    /// Parse flowchart line (node and edge)
    fn parse_flow_line(&self, line: &str, spec: &mut ControlFlowSpec) -> Result<(), MermaidError> {
        // Split by arrow
        let parts: Vec<&str> = if line.contains("-->") {
            line.split("-->").collect()
        } else {
            line.split("---").collect()
        };

        for (i, part) in parts.iter().enumerate() {
            let (node_id, label, node_type) = self.parse_flow_node(part.trim())?;

            // Add node if not exists
            if !spec.nodes.iter().any(|n| n.id == node_id) {
                spec.nodes.push(FlowNode {
                    id: node_id.clone(),
                    label,
                    node_type,
                });
            }

            // Add edge to next node
            if i < parts.len() - 1 {
                let (next_id, _, _) = self.parse_flow_node(parts[i + 1].trim())?;
                spec.edges.push(FlowEdge {
                    from: node_id.clone(),
                    to: next_id,
                    label: None,
                    condition: None,
                });
            }
        }

        Ok(())
    }

    /// Parse flow node
    fn parse_flow_node(&self, s: &str) -> Result<(String, String, FlowNodeType), MermaidError> {
        // Formats:
        // A[Label] - process
        // A{Label} - decision
        // A([Label]) - start/end
        // A((Label)) - circle
        // A>Label] - flag

        let s = s.trim();

        // Extract edge label if present (|label|)
        let s = if let Some(pipe) = s.find('|') {
            &s[..pipe]
        } else {
            s
        };

        let s = s.trim();

        // Find bracket type
        if let Some(bracket_start) = s.find(|c| c == '[' || c == '{' || c == '(') {
            let id = s[..bracket_start].trim().to_string();
            let bracket_char = s.chars().nth(bracket_start).unwrap();

            let (label, node_type) = match bracket_char {
                '[' => {
                    let end = s.rfind(']').unwrap_or(s.len());
                    (s[bracket_start + 1..end].to_string(), FlowNodeType::Process)
                }
                '{' => {
                    let end = s.rfind('}').unwrap_or(s.len());
                    (
                        s[bracket_start + 1..end].to_string(),
                        FlowNodeType::Decision,
                    )
                }
                '(' => {
                    let end = s.rfind(')').unwrap_or(s.len());
                    let inner = &s[bracket_start + 1..end];
                    if inner.starts_with('(') && inner.ends_with(')') {
                        (inner[1..inner.len() - 1].to_string(), FlowNodeType::End)
                    } else if inner.starts_with('[') && inner.ends_with(']') {
                        (inner[1..inner.len() - 1].to_string(), FlowNodeType::Start)
                    } else {
                        (inner.to_string(), FlowNodeType::Start)
                    }
                }
                _ => (s.to_string(), FlowNodeType::Process),
            };

            Ok((id, label, node_type))
        } else {
            Ok((s.to_string(), s.to_string(), FlowNodeType::Process))
        }
    }
}
