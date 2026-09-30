use crate::domain::diagnostic::model::DiagnosticSeverity;
use crate::domain::syntax::language::Language;
use std::path::Path;

/// Lint configuration: which languages to check, which paths to skip, and the
/// lowest severity to report.
///
/// Start from [`LintConfig::default`] and adjust it with the `with_*`
/// builders.
#[derive(Debug, Clone)]
pub struct LintConfig {
    languages: Vec<Language>,
    exclude_patterns: Vec<String>,
    min_severity: DiagnosticSeverity,
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
    /// Replace the languages to check.
    pub fn with_languages(mut self, languages: impl IntoIterator<Item = Language>) -> Self {
        self.languages = languages.into_iter().collect();
        self
    }

    /// Replace the path substrings that exclude a file from checking.
    pub fn with_exclude_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.exclude_patterns = patterns.into_iter().map(Into::into).collect();
        self
    }

    /// Set the lowest severity to report.
    pub fn with_min_severity(mut self, min_severity: DiagnosticSeverity) -> Self {
        self.min_severity = min_severity;
        self
    }

    /// The languages to check.
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// The path substrings that exclude a file from checking.
    pub fn exclude_patterns(&self) -> &[String] {
        &self.exclude_patterns
    }

    /// The lowest severity to report.
    pub fn min_severity(&self) -> DiagnosticSeverity {
        self.min_severity
    }

    pub fn is_language_enabled(&self, lang: Language) -> bool {
        self.languages.contains(&lang)
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude_patterns.iter().any(|p| path_str.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_replace_the_defaults() {
        let config = LintConfig::default()
            .with_languages([Language::Go])
            .with_exclude_patterns(["vendor"])
            .with_min_severity(DiagnosticSeverity::Hint);
        assert_eq!(config.languages(), &[Language::Go]);
        assert_eq!(config.exclude_patterns(), &["vendor".to_string()]);
        assert_eq!(config.min_severity(), DiagnosticSeverity::Hint);
        assert!(config.is_language_enabled(Language::Go));
        assert!(!config.is_language_enabled(Language::Python));
        assert!(config.is_excluded(Path::new("a/vendor/b.go")));
    }
}
