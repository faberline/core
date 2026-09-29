//! Project-wide analysis
//!
//! This module provides:
//! - Python file discovery (respects .gitignore)
//! - pyproject.toml configuration reading
//! - Directory exclusion (venv, __pycache__, .git)
//! - Project-wide type checking

pub use crate::infrastructure::project::analyzer::ProjectAnalyzer;
pub use crate::infrastructure::project::config::ProjectConfig;
