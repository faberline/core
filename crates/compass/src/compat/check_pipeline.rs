//! Check pipeline with cross-file type propagation (R4, R5, R10).
//!
//! Wires `PropagationPipeline` into the analysis flow so that:
//! - `type_at` queries on imported symbols return the propagated type (R4)
//! - `hover` queries on imported symbols return the propagated type signature (R5)
//! - `check_paths` diagnostics benefit from cross-file types (R10)
//!
//! This is the integration layer between per-file type inference and
//! cross-file propagation.  After lens dissolution this replaces the
//! former `lens/mod.rs` check pipeline.

pub use crate::application::check::pipeline::{
    hover, recheck_after_change, run_check_pipeline, type_at, HoverResult, TypeAtResult,
};
