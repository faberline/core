//! Deep cross-file type inference (Sprint 2 - Track 1)
//!
//! Provides advanced type inference capabilities:
//! - Cross-file type tracking and propagation
//! - Full generic and TypeVar support
//! - Protocol and structural typing
//! - Advanced type narrowing
//! - Recursive type handling

pub use crate::domain::cross_file::binding::{
    GenericKey, MethodSignature, ProtocolDef, TypeBinding, TypeVarInfo,
};
pub use crate::domain::cross_file::context::TypeContext;
pub use crate::domain::cross_file::file_analysis::{FileAnalysis, ImportInfo};
pub use crate::domain::cross_file::import_graph::ImportGraph;
pub use crate::domain::cross_file::inferencer::trace::{
    infer_type_deep, trace_type_chain, CrossFileRef, DeepInferenceResult, TypeTraceStep,
};
pub use crate::domain::cross_file::inferencer::DeepTypeInferencer;
