use crate::domain::config::argus_config::ArgusConfig;
use std::path::Path;

impl ArgusConfig {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_str(&content)
    }

    /// Parse configuration from a string
    pub fn from_str(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content).map_err(ConfigError::Parse)
    }

    /// Find and load configuration from a directory (looks for cclab_lens.toml)
    pub fn from_directory(dir: &Path) -> Result<Self, ConfigError> {
        let config_path = dir.join("cclab_lens.toml");
        if config_path.exists() {
            Self::from_file(&config_path)
        } else {
            // Try parent directories
            if let Some(parent) = dir.parent() {
                Self::from_directory(parent)
            } else {
                // No config found, use defaults
                Ok(Self::default())
            }
        }
    }
}

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "Failed to read config file: {}", e),
            ConfigError::Parse(e) => write!(f, "Failed to parse config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests;
