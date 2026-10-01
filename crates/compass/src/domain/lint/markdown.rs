use std::path::PathBuf;

use super::checker::Checker;
use crate::checker::LintConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::syntax::{Language, ParsedFile};

mod line_rules;
mod link_rules;
mod structure_rules;
pub(crate) mod symbol;

// ============================================================================
// MarkdownChecker
// ============================================================================

/// Markdown checker (structural analysis with 10 built-in rules + MD011)
pub struct MarkdownChecker {
    /// Optional workspace root for broken-link checks (MD011).
    workspace_root: Option<PathBuf>,
}

impl MarkdownChecker {
    /// Create a new checker (no workspace root — MD011 disabled).
    pub fn new() -> Self {
        Self {
            workspace_root: None,
        }
    }

    /// Create a checker with a workspace root, enabling MD011 broken-link checks.
    pub fn with_workspace(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root: Some(workspace_root),
        }
    }
}

impl Default for MarkdownChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a single-line Range for a given 0-indexed line number.
pub(crate) fn line_range(line_num: u32) -> Range {
    Range::new(
        Position::new(line_num, 0),
        Position::new(line_num, u32::MAX),
    )
}

impl Checker for MarkdownChecker {
    fn language(&self) -> Language {
        Language::Markdown
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let mut in_code_block = false;
        let mut in_frontmatter = false;
        let mut last_heading_level: Option<usize> = None;
        let mut seen_headings: Vec<String> = Vec::new();
        let mut consecutive_blank: u32 = 0;
        // Tracks whether frontmatter appears to be malformed (no closing ---)
        let mut frontmatter_line_start: Option<u32> = None;

        for (line_idx, line) in file.source.lines().enumerate() {
            let line_num = line_idx as u32;

            // --- Frontmatter detection (first block only) ---
            if line_num == 0 && line.trim() == "---" {
                in_frontmatter = true;
                frontmatter_line_start = Some(0);
                continue;
            }

            if in_frontmatter {
                if line.trim() == "---" || line.trim() == "..." {
                    in_frontmatter = false;
                    frontmatter_line_start = None;

                    // MD008: frontmatter exists — schema validation deferred
                    diagnostics.push(Diagnostic::new(
                        line_range(line_num),
                        DiagnosticSeverity::Hint,
                        "MD008",
                        DiagnosticCategory::Style,
                        "Frontmatter detected — schema validation delegated to schema registry",
                    ));
                } else {
                    // MD007: basic frontmatter parse check — key: value expected
                    let trimmed = line.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with('#')
                        && !trimmed.starts_with('-')
                        && !trimmed.contains(':')
                    {
                        diagnostics.push(Diagnostic::new(
                            line_range(line_num),
                            DiagnosticSeverity::Warning,
                            "MD007",
                            DiagnosticCategory::Syntax,
                            format!(
                                "Invalid frontmatter line '{}' — expected 'key: value' format",
                                trimmed
                            ),
                        ));
                    }
                }
                continue;
            }

            // --- Code block toggle ---
            let trimmed = line.trim();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                if in_code_block {
                    in_code_block = false;
                } else {
                    in_code_block = true;
                    // MD003: missing language tag
                    self.check_missing_code_lang(line_num, line, &mut diagnostics);
                }
                continue;
            }

            if in_code_block {
                // Skip all rule checks inside code blocks
                continue;
            }

            // --- Blank line tracking ---
            if trimmed.is_empty() {
                consecutive_blank += 1;
                self.check_consecutive_blanks(line_num, consecutive_blank, &mut diagnostics);
                continue;
            } else {
                consecutive_blank = 0;
            }

            // --- Heading detection ---
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|c| *c == '#').count();
                if level <= 6 {
                    let heading_text = trimmed[level..].trim();

                    // MD001: heading level skip
                    self.check_heading_level_skip(
                        line_num,
                        level,
                        &mut last_heading_level,
                        &mut diagnostics,
                    );

                    // MD002: duplicate heading
                    self.check_duplicate_heading(
                        line_num,
                        heading_text,
                        &mut seen_headings,
                        &mut diagnostics,
                    );

                    continue;
                }
            }

            // --- Per-line rules ---
            self.check_line_length(line_num, line, &mut diagnostics);
            self.check_relative_links(line_num, line, &mut diagnostics);
            self.check_external_links(line_num, line, &mut diagnostics);
            self.check_trailing_whitespace(line_num, line, &mut diagnostics);
        }

        // MD007: unclosed frontmatter
        if in_frontmatter {
            if let Some(start) = frontmatter_line_start {
                diagnostics.push(Diagnostic::new(
                    line_range(start),
                    DiagnosticSeverity::Warning,
                    "MD007",
                    DiagnosticCategory::Syntax,
                    "Frontmatter block opened with '---' but never closed",
                ));
            }
        }

        // MD011: Broken relative link — verify file existence when workspace
        // root is known.
        if let Some(ref root) = self.workspace_root {
            self.check_broken_links(root, &file.source, &mut diagnostics);
        }

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "MD001", // Heading level skip
            "MD002", // Duplicate heading text
            "MD003", // Missing code block language tag
            "MD004", // Line length > 120 chars
            "MD005", // Relative internal link (hint)
            "MD006", // External link (info)
            "MD007", // Invalid / unclosed frontmatter
            "MD008", // Frontmatter schema (deferred)
            "MD009", // Trailing whitespace
            "MD010", // Multiple consecutive blank lines
            "MD011", // Broken relative link (file not found)
        ]
    }
}

#[cfg(test)]
mod tests;
