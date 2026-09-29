//! Scope analysis for Python code
//!
//! Tracks variable definitions and usages across scopes to detect:
//! - Unused variables (PY103)
//! - Undefined names (PY105)
//! - Variable redeclaration (PY106)

mod analyzer;

use crate::domain::diagnostic::model::Range;
use std::collections::HashMap;

pub use analyzer::ScopeAnalyzer;

/// Kind of symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Function,
    Class,
    Import,
    Global,
    Nonlocal,
}

/// A symbol in a scope
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub defined_at: Range,
    pub used: bool,
    pub assigned_multiple: bool,
}

/// Kind of scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Module,
    Function,
    Class,
    Comprehension,
    Lambda,
}

/// A scope containing symbols
#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub symbols: HashMap<String, Symbol>,
    pub parent: Option<usize>, // Index of parent scope
}

impl Scope {
    pub fn new(kind: ScopeKind, parent: Option<usize>) -> Self {
        Self {
            kind,
            symbols: HashMap::new(),
            parent,
        }
    }

    pub fn define(&mut self, name: String, kind: SymbolKind, range: Range) {
        if let Some(existing) = self.symbols.get_mut(&name) {
            existing.assigned_multiple = true;
        } else {
            self.symbols.insert(
                name.clone(),
                Symbol {
                    name,
                    kind,
                    defined_at: range,
                    used: false,
                    assigned_multiple: false,
                },
            );
        }
    }

    pub fn mark_used(&mut self, name: &str) -> bool {
        if let Some(symbol) = self.symbols.get_mut(name) {
            symbol.used = true;
            true
        } else {
            false
        }
    }
}
