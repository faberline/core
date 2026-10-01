use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod flowchart_rules;

const VALID_DIAGRAM_TYPES: &[&str] = &[
    "graph",
    "flowchart",
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "erDiagram",
    "gantt",
    "pie",
    "mindmap",
    "timeline",
    "gitgraph",
    "journey",
    "quadrantChart",
    "requirementDiagram",
    "C4Context",
];

/// Mermaid checker — line-based, no tree-sitter
pub struct MermaidChecker;

impl MermaidChecker {
    pub fn new() -> Self {
        Self
    }

    /// Extract the first non-empty, non-comment token from a line.
    fn first_token(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            return None;
        }
        trimmed.split_whitespace().next()
    }

    /// Return the diagram type from the first meaningful line, or None.
    fn diagram_type<'a>(lines: &[&'a str]) -> Option<&'a str> {
        for line in lines {
            if let Some(tok) = Self::first_token(line) {
                return Some(tok);
            }
        }
        None
    }

    /// MM001: Unknown diagram type
    fn check_unknown_diagram(&self, lines: &[&str]) -> Vec<Diagnostic> {
        let Some(dtype) = Self::diagram_type(lines) else {
            return Vec::new();
        };
        // Allow directives like `%%{init: ...}%%`
        if dtype.starts_with("%%") {
            return Vec::new();
        }
        // Strip optional direction suffix: `graph LR` → dtype = "graph"
        let base = dtype.split_whitespace().next().unwrap_or(dtype);
        // Case-sensitive match — Mermaid diagram types are case-sensitive
        if !VALID_DIAGRAM_TYPES.contains(&base) {
            // Find line index
            let line_num = lines
                .iter()
                .position(|l| l.trim().starts_with(base))
                .unwrap_or(0) as u32;
            return vec![Diagnostic::error(
                Range::new(Position::new(line_num, 0), Position::new(line_num, base.len() as u32)),
                "MM001",
                DiagnosticCategory::Syntax,
                format!("Unknown Mermaid diagram type '{}'. Expected one of: graph, flowchart, sequenceDiagram, …", base),
            )];
        }
        Vec::new()
    }

    /// MM004: Empty diagram — only diagram type declaration, no content
    fn check_empty_diagram(&self, lines: &[&str]) -> Vec<Diagnostic> {
        let mut content_lines = 0usize;
        let mut found_type = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
                continue;
            }
            if !found_type {
                found_type = true;
                // Check it is a valid diagram type declaration
                let base = trimmed.split_whitespace().next().unwrap_or("");
                if VALID_DIAGRAM_TYPES.contains(&base) {
                    continue;
                }
            }
            content_lines += 1;
        }

        if found_type && content_lines == 0 {
            vec![Diagnostic::warning(
                Range::new(Position::new(0, 0), Position::new(0, 1)),
                "MM004",
                DiagnosticCategory::Logic,
                "Diagram has no content — add nodes or connections",
            )]
        } else {
            Vec::new()
        }
    }

    /// MM005: Basic syntax errors — mismatched brackets, arrows without endpoints
    fn check_syntax_errors(&self, lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
                continue;
            }
            let line_num = line_idx as u32;

            // Check mismatched brackets
            let open_square = trimmed.matches('[').count();
            let close_square = trimmed.matches(']').count();
            let open_paren = trimmed.matches('(').count();
            let close_paren = trimmed.matches(')').count();
            let open_curly = trimmed.matches('{').count();
            let close_curly = trimmed.matches('}').count();

            if open_square != close_square {
                diagnostics.push(Diagnostic::new(
                    Range::new(
                        Position::new(line_num, 0),
                        Position::new(line_num, trimmed.len() as u32),
                    ),
                    DiagnosticSeverity::Error,
                    "MM005",
                    DiagnosticCategory::Syntax,
                    "Mismatched square brackets '[' ']'".to_string(),
                ));
            }
            if open_paren != close_paren {
                diagnostics.push(Diagnostic::new(
                    Range::new(
                        Position::new(line_num, 0),
                        Position::new(line_num, trimmed.len() as u32),
                    ),
                    DiagnosticSeverity::Error,
                    "MM005",
                    DiagnosticCategory::Syntax,
                    "Mismatched parentheses '(' ')'".to_string(),
                ));
            }
            if open_curly != close_curly {
                diagnostics.push(Diagnostic::new(
                    Range::new(
                        Position::new(line_num, 0),
                        Position::new(line_num, trimmed.len() as u32),
                    ),
                    DiagnosticSeverity::Error,
                    "MM005",
                    DiagnosticCategory::Syntax,
                    "Mismatched curly braces '{' '}'".to_string(),
                ));
            }

            // Arrow without endpoints: line starts with an arrow token
            let arrow_only = trimmed.starts_with("-->")
                || trimmed.starts_with("---")
                || trimmed.starts_with("==>");
            if arrow_only {
                diagnostics.push(Diagnostic::error(
                    Range::new(
                        Position::new(line_num, 0),
                        Position::new(line_num, trimmed.len() as u32),
                    ),
                    "MM005",
                    DiagnosticCategory::Syntax,
                    "Arrow without a source node endpoint".to_string(),
                ));
            }
        }

        diagnostics
    }
}

impl Default for MermaidChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for MermaidChecker {
    fn language(&self) -> Language {
        Language::Mermaid
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let lines: Vec<&str> = file.source.lines().collect();
        let mut diagnostics = Vec::new();

        diagnostics.extend(self.check_unknown_diagram(&lines));
        diagnostics.extend(self.check_empty_diagram(&lines));
        diagnostics.extend(self.check_syntax_errors(&lines));

        // Node-level checks only make sense once we know the diagram type
        if let Some(dtype) = Self::diagram_type(&lines) {
            diagnostics.extend(self.check_undefined_nodes(&lines, dtype));
            diagnostics.extend(self.check_duplicate_nodes(&lines, dtype));
        }

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "MM001", // Unknown diagram type
            "MM002", // Undefined node reference
            "MM003", // Duplicate node ID
            "MM004", // Empty diagram
            "MM005", // Syntax error (mismatched brackets, arrows without endpoints)
        ]
    }
}

#[cfg(test)]
mod tests;
