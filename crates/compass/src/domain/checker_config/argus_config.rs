use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Python environment configuration for module resolution
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PythonEnvConfig {
    /// Additional directories to search for modules
    pub search_paths: Vec<PathBuf>,

    /// Path to the virtual environment to use (overrides auto-detection)
    pub venv_path: Option<PathBuf>,

    /// Whether to ignore site-packages (default: false)
    #[serde(default)]
    pub ignore_site_packages: bool,
}

/// Configuration for Argus type checker
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ArgusConfig {
    /// Python version to check against (e.g., "3.10")
    pub python_version: Option<String>,

    /// Python environment configuration for module resolution
    #[serde(default)]
    pub python: PythonEnvConfig,

    /// Enable strict mode (like mypy --strict)
    pub strict: bool,

    /// Enable strict optional checking (None must be explicit)
    pub strict_optional: bool,

    /// Warn about returning Any from typed function
    pub warn_return_any: bool,

    /// Warn about unused ignores
    pub warn_unused_ignores: bool,

    /// Check untyped functions
    pub check_untyped_defs: bool,

    /// Disallow untyped decorators
    pub disallow_untyped_decorators: bool,

    /// Disallow incomplete function definitions
    pub disallow_incomplete_defs: bool,

    /// Disallow untyped function definitions
    pub disallow_untyped_defs: bool,

    /// Paths to exclude from analysis
    pub exclude: Vec<String>,

    /// Paths to include (overrides exclude)
    pub include: Vec<String>,

    /// Per-directory overrides
    #[serde(default)]
    pub overrides: Vec<OverrideConfig>,

    /// Custom type stub paths
    pub stub_paths: Vec<PathBuf>,

    /// Plugins to enable
    pub plugins: Vec<String>,

    // === Typeshed configuration ===
    /// Custom path to a local typeshed copy (takes precedence over downloads)
    pub typeshed_path: Option<PathBuf>,

    /// Directory to store downloaded typeshed stubs (default: ~/.cache/cclab_lens)
    pub typeshed_cache_dir: Option<PathBuf>,

    /// Disable network requests for typeshed downloads (offline mode)
    #[serde(default)]
    pub typeshed_offline: bool,

    /// Cache TTL in days for typeshed stubs (default: 7)
    #[serde(default = "default_typeshed_ttl")]
    pub typeshed_ttl_days: u32,

    /// Optional commit hash to pin typeshed version
    pub typeshed_commit: Option<String>,

    /// Stub precedence order: "local", "typeshed", "bundled" (default: local > typeshed > bundled)
    #[serde(default = "default_stub_precedence")]
    pub stub_precedence: Vec<String>,
}

fn default_typeshed_ttl() -> u32 {
    7
}

fn default_stub_precedence() -> Vec<String> {
    vec![
        "local".to_string(),
        "typeshed".to_string(),
        "bundled".to_string(),
    ]
}

/// Per-directory configuration override
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct OverrideConfig {
    /// Glob pattern to match files (e.g., "tests/**/*.py")
    pub pattern: String,

    /// Enable strict mode for matching files
    pub strict: Option<bool>,

    /// Check untyped defs in matching files
    pub check_untyped_defs: Option<bool>,

    /// Disallow untyped defs in matching files
    pub disallow_untyped_defs: Option<bool>,

    /// Ignore missing imports for matching files
    pub ignore_missing_imports: Option<bool>,
}

impl ArgusConfig {
    /// Create a new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the Python version for stub resolution (defaults to "3.11")
    pub fn python_version_or_default(&self) -> String {
        self.python_version
            .clone()
            .unwrap_or_else(|| "3.11".to_string())
    }

    /// Create a strict configuration
    pub fn strict() -> Self {
        Self {
            strict: true,
            strict_optional: true,
            warn_return_any: true,
            warn_unused_ignores: true,
            check_untyped_defs: true,
            disallow_untyped_decorators: true,
            disallow_incomplete_defs: true,
            disallow_untyped_defs: true,
            ..Default::default()
        }
    }

    /// Get effective config for a specific file path
    /// Applies override rules based on glob patterns
    pub fn effective_for(&self, file_path: &Path) -> EffectiveConfig {
        let mut effective = EffectiveConfig {
            strict: self.strict,
            strict_optional: self.strict_optional,
            warn_return_any: self.warn_return_any,
            check_untyped_defs: self.check_untyped_defs,
            disallow_untyped_defs: self.disallow_untyped_defs,
            ignore_missing_imports: false,
        };

        // Apply matching overrides
        let file_str = file_path.to_string_lossy();
        for override_config in &self.overrides {
            if glob_matches(&override_config.pattern, &file_str) {
                if let Some(strict) = override_config.strict {
                    effective.strict = strict;
                }
                if let Some(check) = override_config.check_untyped_defs {
                    effective.check_untyped_defs = check;
                }
                if let Some(disallow) = override_config.disallow_untyped_defs {
                    effective.disallow_untyped_defs = disallow;
                }
                if let Some(ignore) = override_config.ignore_missing_imports {
                    effective.ignore_missing_imports = ignore;
                }
            }
        }

        effective
    }

    /// Check if a path should be excluded from analysis
    pub fn should_exclude(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check explicit excludes
        for pattern in &self.exclude {
            if glob_matches(pattern, &path_str) {
                // Check if explicitly included
                for include_pattern in &self.include {
                    if glob_matches(include_pattern, &path_str) {
                        return false;
                    }
                }
                return true;
            }
        }

        false
    }
}

/// Effective configuration for a specific file
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub strict: bool,
    pub strict_optional: bool,
    pub warn_return_any: bool,
    pub check_untyped_defs: bool,
    pub disallow_untyped_defs: bool,
    pub ignore_missing_imports: bool,
}

/// Simple glob pattern matching
/// Supports * (any characters) and ** (any path segments)
fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace("**", "\x00").replace('*', "[^/]*");
    let pattern = pattern.replace('\x00', ".*");
    let regex_pattern = format!("^{}$", pattern);

    if let Ok(re) = regex::Regex::new(&regex_pattern) {
        re.is_match(path)
    } else {
        // Fallback: simple contains check
        path.contains(pattern.trim_matches('*'))
    }
}

#[cfg(test)]
mod tests;
