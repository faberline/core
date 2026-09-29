use regex_lite::Regex;
use serde::Deserialize;
use tree_sitter::StreamingIterator;

use crate::diagnostic::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, QuickFix, Range,
};
use crate::syntax::ParsedFile;

// ============================================================================
// Rule configuration types (deserialized from rules.toml)
// ============================================================================

/// Matching strategy for a custom rule
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    /// Match a regular expression against each source line
    Regex,
    /// Run a tree-sitter named query against the parsed AST
    Query,
}

/// A single user-defined rule entry from `rules.toml`
#[derive(Debug, Clone, Deserialize)]
pub struct CustomRuleConfig {
    /// Short rule identifier — will be surfaced as `CUSTOM_<id>` in diagnostics
    pub id: String,
    /// Matching strategy
    pub kind: RuleKind,
    /// Regex pattern or tree-sitter query string
    pub pattern: String,
    /// Severity string (`"error"`, `"warning"`, `"information"`, `"hint"`)
    #[serde(default = "default_severity")]
    pub severity: String,
    /// Diagnostic message shown to the user
    pub message: String,
    /// Optional auto-fix hint surfaced as a quick-fix title (no edits)
    #[serde(default)]
    pub fix: Option<String>,
}

fn default_severity() -> String {
    "warning".to_string()
}

impl CustomRuleConfig {
    /// Returns the canonical diagnostic code: `CUSTOM_<ID>`
    pub fn code(&self) -> String {
        format!("CUSTOM_{}", self.id)
    }

    /// Parses the `severity` string into `DiagnosticSeverity`
    pub fn diagnostic_severity(&self) -> DiagnosticSeverity {
        match self.severity.to_ascii_lowercase().as_str() {
            "error" => DiagnosticSeverity::Error,
            "warning" | "warn" => DiagnosticSeverity::Warning,
            "information" | "info" => DiagnosticSeverity::Information,
            "hint" => DiagnosticSeverity::Hint,
            _ => DiagnosticSeverity::Warning,
        }
    }
}

/// Top-level structure of `rules.toml`
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CustomRulesFile {
    /// List of rules (TOML array-of-tables `[[rule]]`)
    #[serde(default, rename = "rule")]
    pub rules: Vec<CustomRuleConfig>,
}

// ============================================================================
// Compiled rule variants
// ============================================================================

struct CompiledRegexRule {
    config: CustomRuleConfig,
    regex: Regex,
}

struct CompiledQueryRule {
    config: CustomRuleConfig,
    /// Raw query string — compiled lazily per-language at check time
    query_str: String,
}

// ============================================================================
// CustomLintEngine
// ============================================================================

/// Engine that evaluates all loaded custom rules against source files.
///
/// Create once with `from_rules_file()` or `load_from_workspace()`, then call
/// `check()` for each file you want to lint.
pub struct CustomLintEngine {
    regex_rules: Vec<CompiledRegexRule>,
    query_rules: Vec<CompiledQueryRule>,
}

impl CustomLintEngine {
    /// Build an engine from an already-parsed `CustomRulesFile`.
    ///
    /// Regex rules are compiled eagerly; invalid patterns are skipped with a
    /// warning rather than panicking.
    pub fn from_rules_file(rules_file: &CustomRulesFile) -> Self {
        let mut regex_rules = Vec::new();
        let mut query_rules = Vec::new();

        for rule in &rules_file.rules {
            match rule.kind {
                RuleKind::Regex => match Regex::new(&rule.pattern) {
                    Ok(regex) => regex_rules.push(CompiledRegexRule {
                        config: rule.clone(),
                        regex,
                    }),
                    Err(e) => {
                        tracing::warn!(
                            "Custom rule '{}': invalid regex '{}': {}",
                            rule.id,
                            rule.pattern,
                            e
                        );
                    }
                },
                RuleKind::Query => query_rules.push(CompiledQueryRule {
                    config: rule.clone(),
                    query_str: rule.pattern.clone(),
                }),
            }
        }

        Self {
            regex_rules,
            query_rules,
        }
    }

    /// Total number of loaded (valid) rules.
    pub fn rule_count(&self) -> usize {
        self.regex_rules.len() + self.query_rules.len()
    }

    /// All custom rule codes exposed by this engine (`CUSTOM_<ID>`).
    pub fn rule_codes(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.regex_rules.iter().map(|r| r.config.code()).collect();
        codes.extend(self.query_rules.iter().map(|r| r.config.code()));
        codes
    }

    /// Apply all custom rules to a parsed file and return diagnostics.
    pub fn check(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        self.apply_regex_rules(file, &mut diagnostics);
        self.apply_query_rules(file, &mut diagnostics);
        diagnostics
    }

    // -----------------------------------------------------------------------
    // Regex rules — operate on raw source lines
    // -----------------------------------------------------------------------

    fn apply_regex_rules(&self, file: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
        for rule in &self.regex_rules {
            self.apply_single_regex_rule(rule, file, diagnostics);
        }
    }

    fn apply_single_regex_rule(
        &self,
        rule: &CompiledRegexRule,
        file: &ParsedFile,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (line_idx, line) in file.source.lines().enumerate() {
            if !rule.regex.is_match(line) {
                continue;
            }

            let line_num = line_idx as u32;
            let range = Range::new(
                Position::new(line_num, 0),
                Position::new(line_num, line.len() as u32),
            );

            let mut diag = Diagnostic::new(
                range,
                rule.config.diagnostic_severity(),
                rule.config.code(),
                DiagnosticCategory::Custom,
                rule.config.message.clone(),
            );

            if let Some(fix_title) = &rule.config.fix {
                diag.quick_fixes.push(QuickFix {
                    title: fix_title.clone(),
                    edits: Vec::new(), // hint only — no edits
                });
            }

            diagnostics.push(diag);
        }
    }

    // -----------------------------------------------------------------------
    // Tree-sitter query rules — require a real AST
    // -----------------------------------------------------------------------

    fn apply_query_rules(&self, file: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
        // Line-based files have a dummy HTML tree — skip query rules entirely.
        if file.is_line_based {
            return;
        }

        let language = file.tree.language();

        for rule in &self.query_rules {
            match tree_sitter::Query::new(&language, &rule.query_str) {
                Ok(query) => {
                    self.apply_single_query_rule(rule, file, &query, diagnostics);
                }
                Err(e) => {
                    // Query may be valid for a different language — skip silently at
                    // debug level to avoid noisy output on polyglot repos.
                    tracing::debug!(
                        "Custom query rule '{}': query compile error for this language: {}",
                        rule.config.id,
                        e
                    );
                }
            }
        }
    }

    fn apply_single_query_rule(
        &self,
        rule: &CompiledQueryRule,
        file: &ParsedFile,
        query: &tree_sitter::Query,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source_bytes = file.source.as_bytes();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, file.tree.root_node(), source_bytes);

        while let Some(m) = matches.next() {
            // Use the first capture node as the diagnostic anchor.
            let Some(capture) = m.captures.first() else {
                continue;
            };

            let node = capture.node;
            let range = Range::from_node(&node);

            let mut diag = Diagnostic::new(
                range,
                rule.config.diagnostic_severity(),
                rule.config.code(),
                DiagnosticCategory::Custom,
                rule.config.message.clone(),
            );

            if let Some(fix_title) = &rule.config.fix {
                diag.quick_fixes.push(QuickFix {
                    title: fix_title.clone(),
                    edits: Vec::new(),
                });
            }

            diagnostics.push(diag);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
