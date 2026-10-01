use std::fs;
use std::path::{Path, PathBuf};

/// Directories to always exclude from analysis
const EXCLUDED_DIRS: &[&str] = &[
    "venv",
    ".venv",
    "env",
    ".env",
    "__pycache__",
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    ".tox",
    ".nox",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "dist",
    "build",
    "*.egg-info",
];

/// Project configuration (from pyproject.toml)
#[derive(Debug, Clone, Default)]
pub struct ProjectConfig {
    /// Project root directory
    pub root: PathBuf,
    /// Source directories to analyze
    pub source_dirs: Vec<PathBuf>,
    /// Directories to exclude
    pub exclude: Vec<String>,
    /// Python version target
    pub python_version: Option<String>,
    /// Strict mode
    pub strict: bool,
    /// Type checking mode: basic, standard, strict, all
    pub type_checking_mode: String,
}

impl ProjectConfig {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: root.clone(),
            source_dirs: vec![root],
            exclude: EXCLUDED_DIRS.iter().map(|s| s.to_string()).collect(),
            python_version: None,
            strict: false,
            type_checking_mode: "standard".to_string(),
        }
    }

    /// Load configuration from pyproject.toml
    pub fn from_pyproject(root: &Path) -> Self {
        let mut config = Self::new(root.to_path_buf());

        let pyproject_path = root.join("pyproject.toml");
        if pyproject_path.exists() {
            if let Ok(content) = fs::read_to_string(&pyproject_path) {
                config.parse_pyproject(&content);
            }
        }

        config
    }

    /// Parse pyproject.toml content
    fn parse_pyproject(&mut self, content: &str) {
        // Simple TOML parsing for [tool.cclab_lens] section
        // Note: For production, use a proper TOML parser

        let mut in_tool_cclab_lens = false;
        let mut in_tool_pyright = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Section headers
            if trimmed == "[tool.cclab_lens]" {
                in_tool_cclab_lens = true;
                in_tool_pyright = false;
                continue;
            } else if trimmed == "[tool.pyright]" || trimmed == "[tool.mypy]" {
                in_tool_cclab_lens = false;
                in_tool_pyright = true;
                continue;
            } else if trimmed.starts_with('[') {
                in_tool_cclab_lens = false;
                in_tool_pyright = false;
                continue;
            }

            // Parse tool.cclab_lens settings
            if in_tool_cclab_lens || in_tool_pyright {
                if let Some((key, value)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');

                    match key {
                        "pythonVersion" | "python_version" => {
                            self.python_version = Some(value.to_string());
                        }
                        "strict" => {
                            self.strict = value == "true";
                        }
                        "typeCheckingMode" | "type_checking_mode" => {
                            self.type_checking_mode = value.to_string();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Check if a path should be excluded
    pub fn should_exclude(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for exclude in &self.exclude {
            if exclude.contains('*') {
                // Glob pattern
                if path_str.contains(&exclude.replace('*', "")) {
                    return true;
                }
            } else if path_str.contains(exclude) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests;
