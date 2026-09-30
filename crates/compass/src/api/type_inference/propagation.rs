//! Cross-file type propagation pipeline.
//!
//! Orchestrates propagation of type bindings across import edges in topological
//! order.  After per-file inference completes, `PropagationPipeline::run`
//! iterates files from leaf dependencies to root entry points, calling
//! `DeepTypeInferencer::propagate_types()` for each import edge so that
//! downstream files receive resolved types instead of `Type::Unknown`.
//!
//! # Requirements
//! - R1: invoke propagate_types() per import edge after per-file inference
//! - R2: topological order — dependencies before dependents
//! - R3: cache propagated types in FileAnalysis.symbols
//! - R7: prefer .pyi stubs over .py sources when present
//! - R8: invalidation + re-propagation on dependency change
//! - R9: detect cycles, mark cycle members, emit diagnostic

pub use crate::application::propagation::pipeline::PropagationPipeline;
pub use crate::application::propagation::types::{
    PropagatedType, PropagationRequest, PropagationResult, PropagationStats,
};
