use super::{line_range, MarkdownChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl MarkdownChecker {
    /// MD001: Heading level skip (e.g. h1 → h3 without h2)
    pub(super) fn check_heading_level_skip(
        &self,
        line_num: u32,
        level: usize,
        last_level: &mut Option<usize>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let Some(prev) = *last_level {
            if level > prev + 1 {
                diagnostics.push(Diagnostic::new(
                    line_range(line_num),
                    DiagnosticSeverity::Warning,
                    "MD001",
                    DiagnosticCategory::Style,
                    format!(
                        "Heading level skipped: h{} after h{} — avoid skipping heading levels",
                        level, prev
                    ),
                ));
            }
        }
        *last_level = Some(level);
    }

    /// MD002: Duplicate heading text in the same file
    pub(super) fn check_duplicate_heading(
        &self,
        line_num: u32,
        text: &str,
        seen: &mut Vec<String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let normalized = text.trim().to_lowercase();
        if seen.iter().any(|h| h == &normalized) {
            diagnostics.push(Diagnostic::new(
                line_range(line_num),
                DiagnosticSeverity::Warning,
                "MD002",
                DiagnosticCategory::Style,
                format!("Duplicate heading text: '{}'", text.trim()),
            ));
        } else {
            seen.push(normalized);
        }
    }

    /// MD003: Code fence without language tag
    pub(super) fn check_missing_code_lang(
        &self,
        line_num: u32,
        line: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Line is exactly "```" or "~~~" (no trailing language)
        let trimmed = line.trim();
        if trimmed == "```" || trimmed == "~~~" {
            diagnostics.push(Diagnostic::new(
                line_range(line_num),
                DiagnosticSeverity::Hint,
                "MD003",
                DiagnosticCategory::Style,
                "Code block is missing a language tag (e.g. ```rust)",
            ));
        }
    }
}
