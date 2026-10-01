//! Configuration system for Argus type checker
//!
//! Reads configuration from pyproject.toml [tool.cclab_lens] section
//! and supports per-directory overrides.
//!
//! ## Python Environment Configuration
//!
//! The `[tool.cclab_lens.python]` section supports:
//! - `search_paths`: Additional directories to search for modules
//! - `venv_path`: Path to the virtual environment to use
//! - `ignore_site_packages`: Whether to ignore site-packages

pub use crate::domain::checker_config::argus_config::{
    ArgusConfig, EffectiveConfig, OverrideConfig, PythonEnvConfig,
};
