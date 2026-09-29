use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Type of virtual environment detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VenvType {
    /// Standard venv created with `python -m venv`
    Venv,
    /// Poetry managed environment
    Poetry,
    /// Pipenv managed environment
    Pipenv,
    /// Conda environment
    Conda,
    /// Unknown or custom environment
    Unknown,
}

impl std::fmt::Display for VenvType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VenvType::Venv => write!(f, "venv"),
            VenvType::Poetry => write!(f, "poetry"),
            VenvType::Pipenv => write!(f, "pipenv"),
            VenvType::Conda => write!(f, "conda"),
            VenvType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Information about a detected virtual environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEnv {
    /// Path to the virtual environment
    pub path: PathBuf,
    /// Type of virtual environment
    pub env_type: VenvType,
    /// Path to the site-packages directory (if found)
    pub site_packages: Option<PathBuf>,
}

/// Comprehensive environment information for Python module resolution
#[derive(Debug, Clone, Default)]
pub struct EnvInfo {
    /// Active virtual environment (from config or auto-detected)
    pub active_venv: Option<DetectedEnv>,
    /// All detected virtual environments in the project
    pub detected_envs: Vec<DetectedEnv>,
    /// Combined search paths in priority order
    pub search_paths: Vec<PathBuf>,
    /// Python version (e.g., "3.11")
    pub python_version: Option<String>,
}
