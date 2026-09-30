//! Code generator traits and configuration
//!
//! Defines the `CodeGenerator` trait that all generators implement,
//! along with `TechStack` enum and `GenContext` configuration.

pub use crate::infrastructure::codegen::traits::{
    CodeGenerator, GenContext, GenError, GenResult, GeneratedCode, Language, TechStack,
};
