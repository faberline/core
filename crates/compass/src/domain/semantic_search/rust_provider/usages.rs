use std::path::PathBuf;

use tree_sitter::Node;

use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::result::{MatchKind, SearchMatch};
use crate::domain::semantic_search::rust_provider::RustSearchProvider;

impl RustSearchProvider {
    /// Find usages of a symbol in Rust code
    pub fn find_usages(
        &self,
        root: &Node,
        source: &str,
        symbol: &str,
        file: &PathBuf,
    ) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        self.visit_for_usages(root, source, symbol, file, &mut matches);
        matches
    }

    fn visit_for_usages(
        &self,
        node: &Node,
        source: &str,
        symbol: &str,
        file: &PathBuf,
        matches: &mut Vec<SearchMatch>,
    ) {
        match node.kind() {
            "identifier" => {
                let text = &source[node.start_byte()..node.end_byte()];
                if text == symbol {
                    matches.push(SearchMatch {
                        file: file.clone(),
                        span: Span {
                            start: node.start_byte(),
                            end: node.end_byte(),
                            start_line: node.start_position().row,
                            start_col: node.start_position().column,
                            end_line: node.end_position().row,
                            end_col: node.end_position().column,
                        },
                        symbol: Some(symbol.to_string()),
                        kind: self.classify_rust_usage(node),
                        score: 1.0,
                        context: None,
                    });
                }
            }
            "type_identifier" => {
                let text = &source[node.start_byte()..node.end_byte()];
                if text == symbol {
                    matches.push(SearchMatch {
                        file: file.clone(),
                        span: Span {
                            start: node.start_byte(),
                            end: node.end_byte(),
                            start_line: node.start_position().row,
                            start_col: node.start_position().column,
                            end_line: node.end_position().row,
                            end_col: node.end_position().column,
                        },
                        symbol: Some(symbol.to_string()),
                        kind: MatchKind::TypeAnnotation,
                        score: 1.0,
                        context: None,
                    });
                }
            }
            _ => {}
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_usages(&child, source, symbol, file, matches);
        }
    }

    /// Classify Rust usage context
    fn classify_rust_usage(&self, node: &Node) -> MatchKind {
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "function_item" => MatchKind::FunctionDef,
                "struct_item" | "enum_item" | "trait_item" => MatchKind::ClassDef,
                "call_expression" | "method_call_expression" => MatchKind::Call,
                "let_declaration" => MatchKind::VariableAssignment,
                "use_declaration" => MatchKind::Import,
                "type_identifier" | "generic_type" => MatchKind::TypeAnnotation,
                _ => MatchKind::VariableAssignment,
            }
        } else {
            MatchKind::VariableAssignment
        }
    }

    /// Find trait implementations
    pub fn find_implementations(
        &self,
        root: &Node,
        source: &str,
        trait_name: &str,
        file: &PathBuf,
    ) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        self.visit_for_impls(root, source, trait_name, file, &mut matches);
        matches
    }

    fn visit_for_impls(
        &self,
        node: &Node,
        source: &str,
        trait_name: &str,
        file: &PathBuf,
        matches: &mut Vec<SearchMatch>,
    ) {
        if node.kind() == "impl_item" {
            // Check if this is an impl for the specified trait
            if let Some(trait_node) = node.child_by_field_name("trait") {
                let impl_trait = &source[trait_node.start_byte()..trait_node.end_byte()];
                // Handle generic traits like Trait<T>
                let base_trait = impl_trait.split('<').next().unwrap_or(impl_trait);

                if base_trait == trait_name {
                    // Get the implementing type
                    let type_name = node
                        .child_by_field_name("type")
                        .map(|t| source[t.start_byte()..t.end_byte()].to_string());

                    matches.push(SearchMatch {
                        file: file.clone(),
                        span: Span {
                            start: node.start_byte(),
                            end: node.end_byte(),
                            start_line: node.start_position().row,
                            start_col: node.start_position().column,
                            end_line: node.end_position().row,
                            end_col: node.end_position().column,
                        },
                        symbol: type_name,
                        kind: MatchKind::ClassDef,
                        score: 1.0,
                        context: None,
                    });
                }
            }
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_impls(&child, source, trait_name, file, matches);
        }
    }
}
