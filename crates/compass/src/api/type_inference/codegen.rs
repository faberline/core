//! Code generation (Sprint 3 - Track 2)
//!
//! Provides type-aware code generation:
//! - Docstring generation from types
//! - Test stub generation
//! - Type stub (pyi) generation
//! - Implementation from interface

pub use crate::domain::type_codegen::generator::CodeGenerator;
pub use crate::domain::type_codegen::request::{
    CodeGenKind, CodeGenOptions, CodeGenRequest, DocstringStyle, TestFramework,
};
pub use crate::domain::type_codegen::result::CodeGenResult;
