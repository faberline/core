//! Top-level file checking orchestrator
//!
//! Provides the public `check_paths` API and supporting types (`FileResult`,
//! `LintConfig`) that were formerly in `lens/mod.rs`.

pub use crate::application::check::check_paths::{check_paths, check_paths_with_propagation};
pub use crate::domain::check::file_result::FileResult;
pub use crate::domain::check::lint_config::LintConfig;
