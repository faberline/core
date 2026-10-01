//! Unified Symbol Table for cross-language semantic analysis
//!
//! Provides a common symbol representation for Python, TypeScript, and Rust.

mod css;
mod dockerfile;
mod gitlab_ci;
mod go;
mod graphql_sym;
mod html;
mod javascript;
mod kubernetes;
mod markdown;
mod mermaid;
mod proto_sym;
mod python;
mod rust;
mod sql_sym;
mod terraform;
mod toml_sym;
mod typescript;

pub use crate::domain::semantic::symbols::{
    Symbol, SymbolId, SymbolKind, SymbolReference, SymbolTable, SymbolTableBuilder, TypeInfo,
};
