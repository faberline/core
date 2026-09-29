//! Python environment detection and configuration
//!
//! This module provides automatic detection of Python virtual environments
//! and configuration of search paths for module resolution.
//!
//! ## Detection Priority
//! 1. Explicit configuration in `pyproject.toml` (`[tool.cclab_lens.python]`)
//! 2. `PYTHONPATH` environment variable
//! 3. Automatic detection of local virtual environments
//! 4. System Python interpreter paths

pub use crate::domain::python_env::environment::{DetectedEnv, EnvInfo, VenvType};
pub use crate::infrastructure::python_env::detect::{
    detect_all_venvs, detect_python_environment, detect_with_config, find_site_packages,
    get_venv_python_version, is_venv_directory,
};
