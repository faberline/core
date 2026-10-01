use crate::diagnostic::Diagnostic;
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

/// Trait for language-specific checkers
pub trait Checker: Send + Sync {
    fn language(&self) -> Language;
    fn check(&self, file: &ParsedFile, config: &LintConfig) -> Vec<Diagnostic>;
    fn available_rules(&self) -> Vec<&'static str>;
}
