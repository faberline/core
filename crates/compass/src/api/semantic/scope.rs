//! Scope analysis for Python code
//!
//! Tracks variable definitions and usages across scopes to detect:
//! - Unused variables (PY103)
//! - Undefined names (PY105)
//! - Variable redeclaration (PY106)

pub use crate::domain::semantic::scope::{Scope, ScopeAnalyzer, ScopeKind, Symbol, SymbolKind};
