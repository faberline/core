//! Semantic Model - Owned, serializable type information independent of AST
//!
//! The SemanticModel provides a persistent representation of resolved types,
//! symbols, and references that can be cached and queried without access to
//! the original source code or AST.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::diagnostic::Range;
use crate::domain::semantic_model::ids::{ScopeId, SymbolId};
use crate::domain::semantic_model::symbol::{SemanticSymbolKind, SymbolData, SymbolReference};
use crate::domain::semantic_model::type_info::TypeInfo;

/// Scope information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    /// Scope ID
    pub id: ScopeId,
    /// Parent scope (None for module scope)
    pub parent: Option<ScopeId>,
    /// Range of the scope in source code
    pub range: Range,
    /// Symbols defined in this scope
    pub symbols: Vec<SymbolId>,
}

/// An interval in the source code that maps to type/symbol information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedRange {
    /// Range in the source code
    pub range: Range,
    /// Type at this range
    pub type_info: TypeInfo,
    /// Symbol ID if this range corresponds to a symbol
    pub symbol_id: Option<SymbolId>,
}

/// The main Semantic Model structure
///
/// Stores resolved types, symbols, and references independent of the AST.
/// This can be serialized and cached for fast retrieval.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticModel {
    /// All symbols indexed by their ID
    pub symbols: HashMap<SymbolId, SymbolData>,
    /// All references to symbols
    pub references: Vec<SymbolReference>,
    /// Scope information
    pub scopes: HashMap<ScopeId, ScopeInfo>,
    /// Type information for ranges (sorted by start position for binary search)
    pub typed_ranges: Vec<TypedRange>,
    /// Symbol lookup by name within scopes
    pub name_to_symbols: HashMap<String, Vec<SymbolId>>,
    /// Next available symbol ID
    next_symbol_id: u64,
    /// Next available scope ID
    next_scope_id: u64,
}

impl SemanticModel {
    /// Create a new empty semantic model
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            references: Vec::new(),
            scopes: HashMap::new(),
            typed_ranges: Vec::new(),
            name_to_symbols: HashMap::new(),
            next_symbol_id: 0,
            next_scope_id: 0,
        }
    }

    /// Allocate a new symbol ID
    pub fn alloc_symbol_id(&mut self) -> SymbolId {
        let id = SymbolId::new(self.next_symbol_id);
        self.next_symbol_id += 1;
        id
    }

    /// Allocate a new scope ID
    pub fn alloc_scope_id(&mut self) -> ScopeId {
        let id = ScopeId::new(self.next_scope_id);
        self.next_scope_id += 1;
        id
    }

    /// Add a symbol to the model
    pub fn add_symbol(&mut self, data: SymbolData) -> SymbolId {
        let id = self.alloc_symbol_id();
        let name = data.name.clone();

        self.symbols.insert(id, data);
        self.name_to_symbols.entry(name).or_default().push(id);

        // Add definition reference
        if let Some(symbol_data) = self.symbols.get(&id) {
            self.references.push(SymbolReference {
                symbol_id: id,
                range: symbol_data.def_range.clone(),
                is_definition: true,
            });
        }

        id
    }

    /// Add a reference to a symbol
    pub fn add_reference(&mut self, symbol_id: SymbolId, range: Range) {
        self.references.push(SymbolReference {
            symbol_id,
            range,
            is_definition: false,
        });
    }

    /// Add a scope to the model
    pub fn add_scope(&mut self, parent: Option<ScopeId>, range: Range) -> ScopeId {
        let id = self.alloc_scope_id();
        self.scopes.insert(
            id,
            ScopeInfo {
                id,
                parent,
                range,
                symbols: Vec::new(),
            },
        );
        id
    }

    /// Add a typed range to the model
    pub fn add_typed_range(
        &mut self,
        range: Range,
        type_info: TypeInfo,
        symbol_id: Option<SymbolId>,
    ) {
        self.typed_ranges.push(TypedRange {
            range,
            type_info,
            symbol_id,
        });
    }

    /// Sort typed ranges by start position for efficient lookup
    pub fn finalize(&mut self) {
        self.typed_ranges.sort_by(|a, b| {
            let line_cmp = a.range.start.line.cmp(&b.range.start.line);
            if line_cmp == std::cmp::Ordering::Equal {
                a.range.start.character.cmp(&b.range.start.character)
            } else {
                line_cmp
            }
        });
    }

    /// Get type information at a position (line, column)
    pub fn type_at(&self, line: u32, column: u32) -> Option<&TypeInfo> {
        // Binary search for the range containing this position
        for typed_range in &self.typed_ranges {
            if typed_range.range.contains(line, column) {
                return Some(&typed_range.type_info);
            }
        }
        None
    }

    /// Get symbol at a position
    pub fn symbol_at(&self, line: u32, column: u32) -> Option<&SymbolData> {
        // First check references
        for reference in &self.references {
            if reference.range.contains(line, column) {
                return self.symbols.get(&reference.symbol_id);
            }
        }

        // Then check symbol definitions
        for symbol in self.symbols.values() {
            if symbol.def_range.contains(line, column) {
                return Some(symbol);
            }
        }

        None
    }

    /// Get the definition of the symbol at a position
    pub fn definition_at(&self, line: u32, column: u32) -> Option<&SymbolData> {
        // Find reference at position
        for reference in &self.references {
            if reference.range.contains(line, column) {
                return self.symbols.get(&reference.symbol_id);
            }
        }
        None
    }

    /// Find all references to the symbol at a position
    pub fn references_at(
        &self,
        line: u32,
        column: u32,
        include_definition: bool,
    ) -> Vec<&SymbolReference> {
        // Find the symbol at position
        let symbol_id = self
            .references
            .iter()
            .find(|r| r.range.contains(line, column))
            .map(|r| r.symbol_id);

        let Some(id) = symbol_id else {
            return Vec::new();
        };

        // Find all references to this symbol
        self.references
            .iter()
            .filter(|r| r.symbol_id == id && (include_definition || !r.is_definition))
            .collect()
    }

    /// Get symbols by name
    pub fn symbols_by_name(&self, name: &str) -> Vec<&SymbolData> {
        self.name_to_symbols
            .get(name)
            .map(|ids| ids.iter().filter_map(|id| self.symbols.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all symbols
    pub fn all_symbols(&self) -> impl Iterator<Item = &SymbolData> {
        self.symbols.values()
    }

    /// Get hover content for symbol at position
    pub fn hover_at(&self, line: u32, column: u32) -> Option<String> {
        let symbol = self.symbol_at(line, column)?;

        let mut content = String::new();
        content.push_str("```python\n");

        match symbol.kind {
            SemanticSymbolKind::Function | SemanticSymbolKind::Method => {
                content.push_str(&format!(
                    "def {}(...) -> {}\n",
                    symbol.name,
                    symbol.type_info.display()
                ));
            }
            SemanticSymbolKind::Class => {
                content.push_str(&format!("class {}\n", symbol.name));
            }
            SemanticSymbolKind::Variable
            | SemanticSymbolKind::Parameter
            | SemanticSymbolKind::Attribute => {
                content.push_str(&format!(
                    "{}: {}\n",
                    symbol.name,
                    symbol.type_info.display()
                ));
            }
            _ => {
                content.push_str(&format!(
                    "{} {}: {}\n",
                    symbol.kind.display_name(),
                    symbol.name,
                    symbol.type_info.display()
                ));
            }
        }

        content.push_str("```");

        if let Some(ref doc) = symbol.documentation {
            content.push_str("\n\n---\n\n");
            content.push_str(doc);
        }

        Some(content)
    }

    /// Get the number of indexed files (symbols count as proxy)
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Merge another semantic model into this one
    pub fn merge(&mut self, other: SemanticModel) {
        // Remap IDs from the other model
        let symbol_id_offset = self.next_symbol_id;
        let scope_id_offset = self.next_scope_id;

        for (old_id, mut symbol) in other.symbols {
            let new_id = SymbolId::new(old_id.0 + symbol_id_offset);
            symbol.scope_id = ScopeId::new(symbol.scope_id.0 + scope_id_offset);
            if let Some(ref mut parent) = symbol.parent_id {
                *parent = SymbolId::new(parent.0 + symbol_id_offset);
            }
            self.symbols.insert(new_id, symbol);
        }

        for mut reference in other.references {
            reference.symbol_id = SymbolId::new(reference.symbol_id.0 + symbol_id_offset);
            self.references.push(reference);
        }

        for (old_id, mut scope) in other.scopes {
            let new_id = ScopeId::new(old_id.0 + scope_id_offset);
            scope.id = new_id;
            if let Some(ref mut parent) = scope.parent {
                *parent = ScopeId::new(parent.0 + scope_id_offset);
            }
            for symbol_id in &mut scope.symbols {
                *symbol_id = SymbolId::new(symbol_id.0 + symbol_id_offset);
            }
            self.scopes.insert(new_id, scope);
        }

        for mut typed_range in other.typed_ranges {
            if let Some(ref mut id) = typed_range.symbol_id {
                *id = SymbolId::new(id.0 + symbol_id_offset);
            }
            self.typed_ranges.push(typed_range);
        }

        for (name, ids) in other.name_to_symbols {
            let entry = self.name_to_symbols.entry(name).or_default();
            for id in ids {
                entry.push(SymbolId::new(id.0 + symbol_id_offset));
            }
        }

        self.next_symbol_id += other.next_symbol_id;
        self.next_scope_id += other.next_scope_id;
    }
}

#[cfg(test)]
mod tests;
