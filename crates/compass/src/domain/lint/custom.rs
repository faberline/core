use crate::domain::syntax::parsed_file::NodeRange;
use serde::Deserialize;
use tree_sitter::StreamingIterator;

use crate::diagnostic::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, QuickFix, Range, RuleCode,
};
use crate::syntax::ParsedFile;

mod pattern;

use pattern::RulePattern;

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

fn custom_code(id: &str) -> RuleCode {
    RuleCode::from(format!("CUSTOM_{id}"))
}

impl CustomRuleConfig {
    /// Returns the canonical diagnostic code: `CUSTOM_<ID>`
    pub fn code(&self) -> RuleCode {
        custom_code(&self.id)
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

/// A regex rule whose pattern does not compile, and why. The engine skips it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRule {
    id: String,
    pattern: String,
    reason: String,
}

impl RejectedRule {
    /// The rule's `id` in `rules.toml`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The code the rule's diagnostics would have had: `CUSTOM_<ID>`.
    pub fn code(&self) -> RuleCode {
        custom_code(&self.id)
    }

    /// The pattern that does not compile.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Why the pattern does not compile.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

// ============================================================================
// Compiled rule variants
// ============================================================================

struct CompiledRegexRule {
    config: CustomRuleConfig,
    regex: RulePattern,
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
    rejected: Vec<RejectedRule>,
}

impl CustomLintEngine {
    /// Build an engine from an already-parsed `CustomRulesFile`.
    ///
    /// Regex rules are compiled eagerly; rules whose pattern does not compile
    /// are skipped and listed by [`rejected_rules`](Self::rejected_rules).
    ///
    /// Patterns keep regex-lite's dialect: `\d`, `\s`, `\w`, word boundaries
    /// and `(?i)` are ASCII-only.
    pub fn from_rules_file(rules_file: &CustomRulesFile) -> Self {
        let mut regex_rules = Vec::new();
        let mut query_rules = Vec::new();
        let mut rejected = Vec::new();

        for rule in &rules_file.rules {
            match rule.kind {
                RuleKind::Regex => match RulePattern::new(&rule.pattern) {
                    Ok(regex) => regex_rules.push(CompiledRegexRule {
                        config: rule.clone(),
                        regex,
                    }),
                    Err(e) => rejected.push(RejectedRule {
                        id: rule.id.clone(),
                        pattern: rule.pattern.clone(),
                        reason: e.to_string(),
                    }),
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
            rejected,
        }
    }

    /// The regex rules skipped because their pattern does not compile.
    pub fn rejected_rules(&self) -> &[RejectedRule] {
        &self.rejected
    }

    /// Total number of loaded (valid) rules.
    pub fn rule_count(&self) -> usize {
        self.regex_rules.len() + self.query_rules.len()
    }

    /// All custom rule codes exposed by this engine (`CUSTOM_<ID>`).
    pub fn rule_codes(&self) -> Vec<RuleCode> {
        let mut codes: Vec<RuleCode> = self.regex_rules.iter().map(|r| r.config.code()).collect();
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
            // A query may be valid only for another language: skip it for this
            // file, silently, to avoid noisy output on polyglot repos.
            if let Ok(query) = tree_sitter::Query::new(&language, &rule.query_str) {
                self.apply_single_query_rule(rule, file, &query, diagnostics);
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
            let range = node.to_range();

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
