use crate::domain::diagnostic::model::DiagnosticSeverity;
use crate::domain::syntax::language::Language;
use std::path::Path;

/// Lint configuration
#[derive(Debug, Clone)]
pub struct LintConfig {
    pub languages: Vec<Language>,
    pub exclude_patterns: Vec<String>,
    pub min_severity: DiagnosticSeverity,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            languages: vec![Language::Python, Language::TypeScript, Language::Rust],
            exclude_patterns: vec![
                "__pycache__".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                ".venv".to_string(),
            ],
            min_severity: DiagnosticSeverity::Warning,
        }
    }
}

impl LintConfig {
    pub fn is_language_enabled(&self, lang: Language) -> bool {
        self.languages.contains(&lang)
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude_patterns.iter().any(|p| path_str.contains(p))
    }
}
