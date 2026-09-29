//! Unified Symbol Table for cross-language semantic analysis
//!
//! Provides a common symbol representation for Python, TypeScript, and Rust.

mod builder;
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
mod symbol;
mod terraform;
#[cfg(test)]
mod tests;
mod toml_sym;
mod type_info;
mod typescript;

use crate::domain::diagnostic::model::Range;
use std::collections::HashMap;

pub use builder::SymbolTableBuilder;
pub use symbol::{Symbol, SymbolId, SymbolKind, SymbolReference};
pub use type_info::TypeInfo;

/// Symbol table for a file
#[derive(Debug, Default)]
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    by_name: HashMap<String, Vec<SymbolId>>,
    references: Vec<SymbolReference>,
    next_id: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol to the table
    pub fn add_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        location: Range,
        type_info: Option<TypeInfo>,
        doc: Option<String>,
        scope_id: usize,
    ) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;

        let symbol = Symbol {
            id,
            name: name.clone(),
            kind,
            location: location.clone(),
            type_info,
            doc,
            scope_id,
        };

        self.symbols.push(symbol);
        self.by_name.entry(name).or_default().push(id);

        // Add definition reference
        self.references.push(SymbolReference {
            symbol_id: id,
            location,
            is_definition: true,
        });

        id
    }

    /// Add a reference to a symbol
    pub fn add_reference(&mut self, symbol_id: SymbolId, location: Range) {
        self.references.push(SymbolReference {
            symbol_id,
            location,
            is_definition: false,
        });
    }

    /// Get symbol by ID
    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0)
    }

    /// Find symbols by name
    pub fn find_by_name(&self, name: &str) -> Vec<&Symbol> {
        self.by_name
            .get(name)
            .map(|ids| ids.iter().filter_map(|id| self.get(*id)).collect())
            .unwrap_or_default()
    }

    /// Find symbol at position
    pub fn find_at_position(&self, line: u32, character: u32) -> Option<&Symbol> {
        // First check references (more precise)
        for reference in &self.references {
            if reference.location.contains(line, character) {
                return self.get(reference.symbol_id);
            }
        }

        // Then check symbol definitions
        for symbol in &self.symbols {
            if symbol.location.contains(line, character) {
                return Some(symbol);
            }
        }

        None
    }

    /// Find definition of symbol at position
    pub fn find_definition_at(&self, line: u32, character: u32) -> Option<&Symbol> {
        // Find what's at position
        for reference in &self.references {
            if reference.location.contains(line, character) {
                return self.get(reference.symbol_id);
            }
        }
        None
    }

    /// Find all references to symbol at position
    pub fn find_references_at(
        &self,
        line: u32,
        character: u32,
        include_definition: bool,
    ) -> Vec<Range> {
        // Find the symbol at position
        let symbol_id = self
            .references
            .iter()
            .find(|r| r.location.contains(line, character))
            .map(|r| r.symbol_id);

        let Some(id) = symbol_id else {
            return Vec::new();
        };

        // Find all references to this symbol
        self.references
            .iter()
            .filter(|r| r.symbol_id == id && (include_definition || !r.is_definition))
            .map(|r| r.location.clone())
            .collect()
    }

    /// Get all symbols
    pub fn all_symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Get all references (definitions + usages)
    pub fn all_references(&self) -> &[SymbolReference] {
        &self.references
    }
}
