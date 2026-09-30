//! Rust public export scanner for Python-oriented code generation.
//!
//! Parses Rust source files using tree-sitter to extract public items:
//! - `pub struct` (data or stateful)
//! - `pub enum`
//! - `pub fn` / `pub async fn`
//!
//! Convention: public Rust items can be discovered and projected into Python-facing generators.

pub use crate::domain::rust_source_scan::exports::{
    RustEnum, RustEnumVariant, RustExports, RustField, RustFunction, RustMethod, RustParam,
    RustStruct, StructKind,
};
pub use crate::infrastructure::rust_source_scan::scanner::RustScanner;
