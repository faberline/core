mod visit;

use super::{Scope, ScopeKind, Symbol, SymbolKind};
use crate::domain::diagnostic::model::Range;
use crate::domain::syntax::parsed_file::ParsedFile;

/// Scope analyzer for Python
pub struct ScopeAnalyzer {
    scopes: Vec<Scope>,
    current_scope: usize,
}

impl ScopeAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            scopes: Vec::new(),
            current_scope: 0,
        };
        // Create module scope
        analyzer.scopes.push(Scope::new(ScopeKind::Module, None));
        analyzer
    }

    /// Analyze a parsed Python file
    pub fn analyze(&mut self, file: &ParsedFile) {
        self.visit_node(&file.root_node(), file);
    }

    fn push_scope(&mut self, kind: ScopeKind) {
        let parent = Some(self.current_scope);
        self.scopes.push(Scope::new(kind, parent));
        self.current_scope = self.scopes.len() - 1;
    }

    fn pop_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    fn current(&mut self) -> &mut Scope {
        &mut self.scopes[self.current_scope]
    }

    fn visit_children(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(&child, file);
        }
    }

    fn visit_parameters(&mut self, params: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    let name = file.node_text(&child).to_string();
                    self.current()
                        .define(name, SymbolKind::Parameter, Range::from_node(&child));
                }
                "typed_parameter" | "typed_default_parameter" | "default_parameter" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = file.node_text(&name_node).to_string();
                        self.current().define(
                            name,
                            SymbolKind::Parameter,
                            Range::from_node(&name_node),
                        );
                    }
                }
                "list_splat_pattern" | "dictionary_splat_pattern" => {
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "identifier" {
                            let name = file.node_text(&inner).to_string();
                            self.current().define(
                                name,
                                SymbolKind::Parameter,
                                Range::from_node(&inner),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn define_pattern(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        match node.kind() {
            "identifier" => {
                let name = file.node_text(node).to_string();
                self.current()
                    .define(name, SymbolKind::Variable, Range::from_node(node));
            }
            "tuple_pattern" | "list_pattern" | "pattern_list" | "tuple" | "list" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.define_pattern(&child, file);
                }
            }
            "subscript" | "attribute" => {
                // x[0] = ... or x.y = ... - visit right side for uses
                // but don't define the subscript/attribute itself
            }
            _ => {}
        }
    }

    fn mark_used_in_scope(&mut self, name: &str) {
        // Try current scope first, then walk up
        let mut scope_idx = self.current_scope;
        loop {
            if self.scopes[scope_idx].mark_used(name) {
                return;
            }
            match self.scopes[scope_idx].parent {
                Some(parent) => scope_idx = parent,
                None => return,
            }
        }
    }

    /// Get all unused symbols (for PY103)
    pub fn unused_symbols(&self) -> Vec<&Symbol> {
        let mut unused = Vec::new();
        for scope in &self.scopes {
            for symbol in scope.symbols.values() {
                // Skip if used, or if it's a special name
                if symbol.used {
                    continue;
                }
                // Skip _ (intentionally unused)
                if symbol.name == "_" || symbol.name.starts_with('_') {
                    continue;
                }
                // Skip self/cls
                if symbol.name == "self" || symbol.name == "cls" {
                    continue;
                }
                // Only report unused variables and parameters
                if matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Parameter) {
                    unused.push(symbol);
                }
            }
        }
        unused
    }

    /// Get symbols that were redeclared (for PY106)
    pub fn redeclared_symbols(&self) -> Vec<&Symbol> {
        let mut redeclared = Vec::new();
        for scope in &self.scopes {
            for symbol in scope.symbols.values() {
                if symbol.assigned_multiple && matches!(symbol.kind, SymbolKind::Variable) {
                    redeclared.push(symbol);
                }
            }
        }
        redeclared
    }

    /// Check if a function body is a stub (only contains ... or pass, optionally with docstring)
    fn is_stub_body(body: &tree_sitter::Node<'_>, file: &ParsedFile) -> bool {
        let mut cursor = body.walk();
        let children: Vec<_> = body.children(&mut cursor).collect();

        // Empty body
        if children.is_empty() {
            return true;
        }

        // Check each statement in the body
        let mut has_real_code = false;
        for child in &children {
            match child.kind() {
                // Skip docstrings
                "expression_statement" => {
                    if let Some(expr) = child.child(0) {
                        if expr.kind() == "string" {
                            // This is a docstring, continue checking
                            continue;
                        } else if expr.kind() == "ellipsis" {
                            // ... is a stub marker
                            continue;
                        }
                    }
                    has_real_code = true;
                }
                // pass is a stub marker
                "pass_statement" => continue,
                // raise NotImplementedError is also a stub pattern
                "raise_statement" => {
                    let text = file.node_text(child);
                    if text.contains("NotImplementedError") {
                        continue;
                    }
                    has_real_code = true;
                }
                // Comments are OK
                "comment" => continue,
                // Anything else is real code
                _ => {
                    has_real_code = true;
                }
            }
        }

        !has_real_code
    }
}

impl Default for ScopeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
