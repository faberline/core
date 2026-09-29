use serde::Deserialize;
use std::collections::HashSet;

/// Top-level Argus configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArgusConfig {
    #[serde(default)]
    pub cclab_lens: ArgusSettings,
}

/// Main settings under [cclab_lens]
#[derive(Debug, Clone, Deserialize)]
pub struct ArgusSettings {
    /// Languages to analyze
    #[serde(default)]
    pub languages: Vec<String>,

    /// LSP server port (default: 5007)
    #[serde(default = "default_lsp_port")]
    pub lsp_port: u16,

    /// Python-specific settings
    #[serde(default)]
    pub python: PythonConfig,

    /// TypeScript-specific settings
    #[serde(default)]
    pub typescript: TypeScriptConfig,

    /// Rust-specific settings
    #[serde(default)]
    pub rust: RustConfig,
}

impl Default for ArgusSettings {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            lsp_port: default_lsp_port(),
            python: PythonConfig::default(),
            typescript: TypeScriptConfig::default(),
            rust: RustConfig::default(),
        }
    }
}

/// Python configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PythonConfig {
    /// Target Python version (e.g., "3.11")
    #[serde(default)]
    pub target_version: Option<String>,

    /// Patterns to exclude
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Lint settings
    #[serde(default)]
    pub lint: LintConfig,

    /// isort-like settings
    #[serde(default)]
    pub isort: IsortConfig,
}

/// TypeScript configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TypeScriptConfig {
    /// Whether TypeScript checking is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Patterns to exclude
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Lint settings
    #[serde(default)]
    pub lint: LintConfig,
}

/// Rust configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RustConfig {
    /// Whether Rust checking is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Patterns to exclude
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Lint settings
    #[serde(default)]
    pub lint: LintConfig,
}

/// Lint configuration (shared across languages)
#[derive(Debug, Clone, Deserialize)]
pub struct LintConfig {
    /// Whether linting is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Rules to enable (e.g., ["PY1", "PY2", "PY4"])
    #[serde(default)]
    pub select: Vec<String>,

    /// Rules to ignore (e.g., ["PY103"])
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            select: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

/// Language-agnostic lint config used by checkers
#[derive(Debug, Clone, Default)]
pub struct LanguageConfig {
    /// Rules to ignore
    pub ignore_rules: HashSet<String>,
    /// Rule prefixes to select (empty = all)
    pub select_prefixes: Vec<String>,
}

impl LanguageConfig {
    /// Check if a rule is enabled
    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        // If explicitly ignored, skip
        if self.ignore_rules.contains(rule_id) {
            return false;
        }

        // If select is empty, all rules are enabled
        if self.select_prefixes.is_empty() {
            return true;
        }

        // Check if rule matches any select prefix
        self.select_prefixes
            .iter()
            .any(|prefix| rule_id.starts_with(prefix))
    }
}

impl From<&LintConfig> for LanguageConfig {
    fn from(config: &LintConfig) -> Self {
        Self {
            ignore_rules: config.ignore.iter().cloned().collect(),
            select_prefixes: config.select.clone(),
        }
    }
}

/// isort-like configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IsortConfig {
    /// Whether isort is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Known first-party packages
    #[serde(default)]
    pub known_first_party: Vec<String>,

    /// Known third-party packages
    #[serde(default)]
    pub known_third_party: Vec<String>,

    /// Known standard library modules (overrides)
    #[serde(default)]
    pub known_standard_library: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_lsp_port() -> u16 {
    5007
}

impl ArgusConfig {
    /// Get the language config for Python
    pub fn python_lint_config(&self) -> LanguageConfig {
        LanguageConfig::from(&self.cclab_lens.python.lint)
    }

    /// Get the language config for TypeScript
    pub fn typescript_lint_config(&self) -> LanguageConfig {
        LanguageConfig::from(&self.cclab_lens.typescript.lint)
    }

    /// Get the language config for Rust
    pub fn rust_lint_config(&self) -> LanguageConfig {
        LanguageConfig::from(&self.cclab_lens.rust.lint)
    }
}
