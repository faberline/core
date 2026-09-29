use std::path::Path;

use super::{line_range, MarkdownChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl MarkdownChecker {
    /// MD005: Relative link (possible broken internal link — emitted as hint)
    pub(super) fn check_relative_links(
        &self,
        line_num: u32,
        line: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut remaining = line;
        while let Some(open) = remaining.find("](") {
            let after = &remaining[open + 2..];
            let close = after.find(')').unwrap_or(after.len());
            let url = &after[..close];
            // Relative paths start with ./ or ../
            if url.starts_with("./") || url.starts_with("../") {
                diagnostics.push(Diagnostic::new(
                    line_range(line_num),
                    DiagnosticSeverity::Hint,
                    "MD005",
                    DiagnosticCategory::Logic,
                    format!("Relative internal link '{}' — verify path exists", url),
                ));
            }
            remaining = &after[close..];
        }
    }

    /// MD006: External HTTP/HTTPS link (info-level annotation)
    pub(super) fn check_external_links(
        &self,
        line_num: u32,
        line: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut remaining = line;
        while let Some(open) = remaining.find("](") {
            let after = &remaining[open + 2..];
            let close = after.find(')').unwrap_or(after.len());
            let url = &after[..close];
            if url.starts_with("http://") || url.starts_with("https://") {
                diagnostics.push(Diagnostic::new(
                    line_range(line_num),
                    DiagnosticSeverity::Information,
                    "MD006",
                    DiagnosticCategory::Logic,
                    format!(
                        "External link '{}' — consider verifying the URL is reachable",
                        url
                    ),
                ));
            }
            remaining = &after[close..];
        }
    }

    /// MD011: Broken relative link — relative link target does not exist on disk.
    ///
    /// Requires a `workspace_root` to resolve relative paths.
    pub(super) fn check_broken_links(
        &self,
        workspace_root: &Path,
        source: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut in_code_block = false;
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Track code blocks so we skip link detection inside them.
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }

            let line_num = line_idx as u32;
            let mut remaining = line;
            while let Some(open) = remaining.find("](") {
                let after = &remaining[open + 2..];
                let close = after.find(')').unwrap_or(after.len());
                let url = &after[..close];

                // Only check relative paths (./something or ../something)
                if url.starts_with("./") || url.starts_with("../") {
                    // Strip anchors (#section) before checking existence
                    let path_part = url.split('#').next().unwrap_or(url);
                    // Strip query strings
                    let path_part = path_part.split('?').next().unwrap_or(path_part);

                    if !path_part.is_empty() {
                        let full_path = workspace_root.join(path_part);
                        if !full_path.exists() {
                            diagnostics.push(Diagnostic::new(
                                line_range(line_num),
                                DiagnosticSeverity::Error,
                                "MD011",
                                DiagnosticCategory::Logic,
                                format!(
                                    "Broken relative link '{}' — target file does not exist",
                                    url
                                ),
                            ));
                        }
                    }
                }

                remaining = &after[close.min(after.len())..];
            }
        }
    }
}
