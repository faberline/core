use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::checker_config::argus_config::ArgusConfig;

/// pyproject.toml structure
#[derive(Debug, Deserialize)]
struct PyProject {
    tool: Option<ToolSection>,
}

#[derive(Debug, Deserialize)]
struct ToolSection {
    cclab_lens: Option<ArgusConfig>,
}

impl ArgusConfig {
    /// Get the typeshed cache directory (defaults to ~/.cache/cclab_lens)
    pub fn typeshed_cache_dir_or_default(&self) -> PathBuf {
        self.typeshed_cache_dir.clone().unwrap_or_else(|| {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(".cache").join("cclab_lens")
            } else if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
                PathBuf::from(cache).join("cclab_lens")
            } else {
                PathBuf::from(".cclab_lens-cache")
            }
        })
    }

    /// Load config from pyproject.toml in the given directory
    pub fn from_pyproject(dir: &Path) -> Self {
        let pyproject_path = dir.join("pyproject.toml");
        if pyproject_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&pyproject_path) {
                if let Ok(pyproject) = toml::from_str::<PyProject>(&contents) {
                    if let Some(tool) = pyproject.tool {
                        if let Some(config) = tool.cclab_lens {
                            return config;
                        }
                    }
                }
            }
        }
        Self::default()
    }

    /// Find and load config from pyproject.toml by searching up the directory tree
    pub fn discover(start: &Path) -> Self {
        let mut current = start.to_path_buf();
        loop {
            let config = Self::from_pyproject(&current);
            // If we found a config with non-default values, use it
            if config.python_version.is_some()
                || config.strict
                || !config.exclude.is_empty()
                || !config.overrides.is_empty()
            {
                return config;
            }

            // Move up to parent directory
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests;
