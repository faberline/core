//! Versioned language targets and their generated-artifact requirements.
//!
//! A target is intentionally separate from [`crate::Lang`]: `Lang` selects an
//! emitter, while a target selects the minimum language/toolchain contract and
//! any syntax that is safe to use for that contract.

pub use crate::domain::{
    PythonTarget, RustTarget, TargetPolicy, TargetProfile, TargetRequirements, TypeScriptTarget,
};
