use crate::domain::syntax::parsed_file::NodeRange;
use std::collections::HashMap;

use super::CssChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Range};
use crate::syntax::ParsedFile;

impl CssChecker {
    /// CSS001: Detect duplicate selectors in the same stylesheet
    pub(super) fn check_duplicate_selectors(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut seen: HashMap<String, Range> = HashMap::new();
        file.walk(|node, _depth| {
            if node.kind() == "rule_set" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "selectors" {
                        let sel = file.node_text(&child).trim().to_string();
                        if let Some(prev) = seen.get(&sel) {
                            diagnostics.push(Diagnostic::warning(
                                child.to_range(),
                                "CSS001",
                                DiagnosticCategory::Style,
                                format!(
                                    "Duplicate selector '{}' (first at line {})",
                                    sel,
                                    prev.start.line + 1
                                ),
                            ));
                        } else {
                            seen.insert(sel, child.to_range());
                        }
                        break;
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS002: Detect `!important` usage
    pub(super) fn check_important_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "important" {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "CSS002",
                    DiagnosticCategory::Style,
                    "Avoid '!important' — it breaks the natural cascade and is hard to override",
                ));
                return true;
            }
            if node.kind() == "declaration" {
                let text = file.node_text(node);
                if text.contains("!important") {
                    diagnostics.push(Diagnostic::new(
                        node.to_range(),
                        DiagnosticSeverity::Information,
                        "CSS002",
                        DiagnosticCategory::Style,
                        "Avoid '!important' — it breaks the natural cascade",
                    ));
                }
            }
            true
        });
        diagnostics
    }

    /// CSS003: Detect `@import` statements
    pub(super) fn check_import_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "import_statement" {
                diagnostics.push(Diagnostic::warning(
                    node.to_range(),
                    "CSS003",
                    DiagnosticCategory::Style,
                    "Avoid '@import' — use <link> tag or a bundler for better performance",
                ));
            }
            true
        });
        diagnostics
    }

    /// CSS004: Detect empty rule sets
    pub(super) fn check_empty_rules(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "rule_set" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" {
                        let has_decl = {
                            let mut ic = child.walk();
                            let result = child.children(&mut ic).any(|c| {
                                c.kind() == "declaration"
                                    || c.kind() == "rule_set"
                                    || c.kind() == "at_rule"
                            });
                            result
                        };
                        if !has_decl {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "CSS004",
                                DiagnosticCategory::Style,
                                "Empty rule set — remove it or add declarations",
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS005: Detect universal selector `*` usage
    pub(super) fn check_universal_selector(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "universal_selector" {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "CSS005",
                    DiagnosticCategory::Style,
                    "Universal selector '*' may impact performance — use specific selectors",
                ));
            }
            true
        });
        diagnostics
    }
}
