use std::collections::{HashMap, HashSet};

use super::{MermaidChecker, VALID_DIAGRAM_TYPES};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Position, Range};

impl MermaidChecker {
    /// Return true if the diagram is a flowchart/graph type.
    fn is_flowchart(dtype: &str) -> bool {
        matches!(dtype, "graph" | "flowchart")
    }

    /// Extract a node ID from a token like `A`, `A[text]`, `A(text)`, `A{text}`, `A((text))`.
    fn extract_node_id(token: &str) -> Option<&str> {
        // Strip bracket/paren/brace content
        if let Some(pos) = token.find(|c| matches!(c, '[' | '(' | '{' | '>')) {
            let id = &token[..pos];
            if !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                return Some(id);
            }
        }
        // Plain identifier (no brackets)
        if !token.is_empty()
            && token
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Some(token);
        }
        None
    }

    /// Parse flowchart lines for node definitions and edge references.
    /// Returns (defined: Map<id, line>, referenced: Vec<(id, line)>)
    fn parse_flowchart_nodes(lines: &[&str]) -> (HashMap<String, u32>, Vec<(String, u32)>) {
        let mut defined: HashMap<String, u32> = HashMap::new();
        let mut referenced: Vec<(String, u32)> = Vec::new();
        let arrow_patterns = [
            "-->", "---", "-.-", "==>", "-.->", "--o", "--x", "<-->", "o--o",
        ];

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
                continue;
            }
            let line_num = line_idx as u32;

            // Skip diagram type line and directives
            let first = trimmed.split_whitespace().next().unwrap_or("");
            if VALID_DIAGRAM_TYPES.contains(&first)
                || trimmed.starts_with("subgraph")
                || trimmed == "end"
            {
                continue;
            }

            // Check if this line contains an arrow — edge line
            let has_arrow = arrow_patterns.iter().any(|a| trimmed.contains(a));

            if has_arrow {
                // Split on arrow patterns to get endpoints; handles `A --> B` and `A -->|label| B`
                // Simple approach: split on whitespace and look for identifiers before/after arrows
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                let mut i = 0;
                while i < tokens.len() {
                    let tok = tokens[i];
                    // Skip arrow tokens
                    if arrow_patterns.iter().any(|a| {
                        tok.starts_with(a)
                            || tok.ends_with(a)
                            || tok.contains("->")
                            || tok.contains("--")
                    }) {
                        i += 1;
                        continue;
                    }
                    // Skip edge labels like `|text|`
                    if tok.starts_with('|') {
                        i += 1;
                        continue;
                    }
                    // Strip trailing label: `A` from `A -->|label| B`
                    if let Some(id) = Self::extract_node_id(tok) {
                        referenced.push((id.to_string(), line_num));
                        // If has brackets, it's also a definition
                        if tok.contains('[') || tok.contains('(') || tok.contains('{') {
                            defined.entry(id.to_string()).or_insert(line_num);
                        }
                    }
                    i += 1;
                }
            } else {
                // Node definition line: `A[label]` or `A(label)` etc.
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(&first_tok) = tokens.first() {
                    if let Some(id) = Self::extract_node_id(first_tok) {
                        defined.entry(id.to_string()).or_insert(line_num);
                    }
                }
            }
        }

        (defined, referenced)
    }

    /// MM002: Undefined node reference
    pub(super) fn check_undefined_nodes(&self, lines: &[&str], dtype: &str) -> Vec<Diagnostic> {
        if !Self::is_flowchart(dtype) {
            return Vec::new();
        }
        let (defined, referenced) = Self::parse_flowchart_nodes(lines);
        let mut diagnostics = Vec::new();

        // Track which undefined nodes we've already reported to avoid duplicates
        let mut reported: HashSet<String> = HashSet::new();

        for (node_id, line_num) in &referenced {
            if !defined.contains_key(node_id) && !reported.contains(node_id) {
                reported.insert(node_id.clone());
                let col = lines
                    .get(*line_num as usize)
                    .and_then(|l| l.find(node_id.as_str()))
                    .unwrap_or(0) as u32;
                diagnostics.push(Diagnostic::warning(
                    Range::new(
                        Position::new(*line_num, col),
                        Position::new(*line_num, col + node_id.len() as u32),
                    ),
                    "MM002",
                    DiagnosticCategory::Names,
                    format!("Node '{}' is referenced but never defined", node_id),
                ));
            }
        }

        diagnostics
    }

    /// MM003: Duplicate node ID
    pub(super) fn check_duplicate_nodes(&self, lines: &[&str], dtype: &str) -> Vec<Diagnostic> {
        if !Self::is_flowchart(dtype) {
            return Vec::new();
        }

        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut diagnostics = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
                continue;
            }
            let first = trimmed.split_whitespace().next().unwrap_or("");
            if VALID_DIAGRAM_TYPES.contains(&first)
                || trimmed.starts_with("subgraph")
                || trimmed == "end"
            {
                continue;
            }
            // Look for node definitions (lines without arrows)
            let has_arrow = trimmed.contains("-->")
                || trimmed.contains("---")
                || trimmed.contains("-.-")
                || trimmed.contains("==>");
            if !has_arrow {
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(&first_tok) = tokens.first() {
                    // Must have bracket to count as explicit definition
                    if first_tok.contains('[') || first_tok.contains('(') || first_tok.contains('{')
                    {
                        if let Some(id) = Self::extract_node_id(first_tok) {
                            let line_num = line_idx as u32;
                            if let Some(&prev_line) = seen.get(id) {
                                let col = line.find(id).unwrap_or(0) as u32;
                                diagnostics.push(Diagnostic::warning(
                                    Range::new(
                                        Position::new(line_num, col),
                                        Position::new(line_num, col + id.len() as u32),
                                    ),
                                    "MM003",
                                    DiagnosticCategory::Names,
                                    format!(
                                        "Node '{}' is defined multiple times (first at line {})",
                                        id,
                                        prev_line + 1
                                    ),
                                ));
                            } else {
                                seen.insert(id.to_string(), line_num);
                            }
                        }
                    }
                }
            }
        }

        diagnostics
    }
}
