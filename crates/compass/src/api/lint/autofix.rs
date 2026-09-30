//! Auto-fix: apply quick fixes from diagnostics
//!
//! Provides utilities to apply single or batch fixes to source code.

pub use crate::domain::lint::autofix::{apply_all_fixes, apply_fix, FixResult};
