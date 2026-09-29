//! Code generation (Sprint 3 - Track 2)
//!
//! Provides type-aware code generation:
//! - Docstring generation from types
//! - Test stub generation
//! - Type stub (pyi) generation
//! - Implementation from interface

use std::path::PathBuf;

// ============================================================================
// Code Generation Request
// ============================================================================

/// A request for code generation.
#[derive(Debug, Clone)]
pub struct CodeGenRequest {
    /// Type of generation
    pub kind: CodeGenKind,
    /// Source file
    pub file: PathBuf,
    /// Target symbol (function, class, etc.)
    pub symbol: String,
    /// Generation options
    pub options: CodeGenOptions,
}

/// Type of code generation.
#[derive(Debug, Clone)]
pub enum CodeGenKind {
    /// Generate docstring
    Docstring { style: DocstringStyle },
    /// Generate test stubs
    TestStub { framework: TestFramework },
    /// Generate type stub (.pyi) for a symbol
    TypeStub,
    /// Generate type stub for entire module
    ModuleStub,
    /// Generate implementation from protocol/ABC
    Implementation { protocol: String },
    /// Generate constructor (__init__)
    Constructor,
    /// Generate property accessors
    Properties { fields: Vec<String> },
}

/// Docstring style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocstringStyle {
    /// Google style
    Google,
    /// NumPy style
    NumPy,
    /// Sphinx style
    Sphinx,
    /// reStructuredText
    RST,
}

/// Test framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFramework {
    /// pytest
    Pytest,
    /// unittest
    Unittest,
    /// doctest
    Doctest,
}

/// Options for code generation.
#[derive(Debug, Clone, Default)]
pub struct CodeGenOptions {
    /// Include type annotations
    pub include_types: bool,
    /// Include examples in docstrings
    pub include_examples: bool,
    /// Generate async versions
    pub async_support: bool,
    /// Indentation (spaces)
    pub indent: usize,
}
