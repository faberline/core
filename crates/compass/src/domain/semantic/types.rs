//! Language-specific type inference for semantic analysis
//!
//! Provides deeper type analysis beyond what the symbol table captures.

pub(crate) mod go;
pub(crate) mod go_advanced;

#[cfg(test)]
mod go_tests;
