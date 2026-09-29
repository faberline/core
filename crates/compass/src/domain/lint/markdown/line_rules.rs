use super::{line_range, MarkdownChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl MarkdownChecker {
    /// MD004: Line length > 120 chars (skip lines that are pure URLs)
    pub(super) fn check_line_length(
        &self,
        line_num: u32,
        line: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let len = line.len();
        if len > 120 {
            // Skip lines that appear to be bare URLs
            let trimmed = line.trim();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                return;
            }
            diagnostics.push(Diagnostic::new(
                line_range(line_num),
                DiagnosticSeverity::Warning,
                "MD004",
                DiagnosticCategory::Style,
                format!("Line length {} exceeds 120 characters", len),
            ));
        }
    }

    /// MD009: Trailing whitespace on non-empty lines
    pub(super) fn check_trailing_whitespace(
        &self,
        line_num: u32,
        line: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !line.is_empty() && line != line.trim_end() {
            diagnostics.push(Diagnostic::new(
                line_range(line_num),
                DiagnosticSeverity::Warning,
                "MD009",
                DiagnosticCategory::Style,
                "Trailing whitespace",
            ));
        }
    }

    /// MD010: Three or more consecutive blank lines
    pub(super) fn check_consecutive_blanks(
        &self,
        line_num: u32,
        count: u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if count == 3 {
            diagnostics.push(Diagnostic::new(
                line_range(line_num),
                DiagnosticSeverity::Warning,
                "MD010",
                DiagnosticCategory::Style,
                "Multiple consecutive blank lines (3+) — use at most two blank lines",
            ));
        }
    }
}
