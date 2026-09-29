//! Rust-specific semantic search support
//!
//! Adds Rust language support to the unified semantic search API.

use std::collections::HashMap;
use std::path::PathBuf;

use tree_sitter::Node;

use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::engine::CallSite;

mod usages;

/// Rust semantic search provider
pub struct RustSearchProvider;

impl RustSearchProvider {
    pub fn new() -> Self {
        Self
    }

    /// Extract docstrings from Rust code (/// and //! comments)
    pub fn extract_docstrings(&self, root: &Node, source: &str) -> HashMap<String, String> {
        let mut docstrings = HashMap::new();
        self.visit_for_docstrings(root, source, &mut docstrings);
        docstrings
    }

    fn visit_for_docstrings(
        &self,
        node: &Node,
        source: &str,
        docstrings: &mut HashMap<String, String>,
    ) {
        match node.kind() {
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" => {
                // Get name
                if let Some(name_node) = node.child_by_field_name("name") {
                    let symbol_name = &source[name_node.start_byte()..name_node.end_byte()];

                    // Look for doc comments before this node
                    if let Some(doc) = self.extract_rust_doc_comment(node, source) {
                        docstrings.insert(symbol_name.to_string(), doc);
                    }
                }
            }
            _ => {}
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_docstrings(&child, source, docstrings);
        }
    }

    /// Extract Rust doc comment (/// or //!) before a node
    fn extract_rust_doc_comment(&self, node: &Node, source: &str) -> Option<String> {
        let mut comments = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            if sibling.kind() == "line_comment" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                if text.starts_with("///") || text.starts_with("//!") {
                    // Doc comment - prepend (we're going backwards)
                    let cleaned = text
                        .trim_start_matches("///")
                        .trim_start_matches("//!")
                        .trim();
                    comments.insert(0, cleaned.to_string());
                } else {
                    // Regular comment - stop
                    break;
                }
            } else if sibling.kind() == "block_comment" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                if text.starts_with("/**") || text.starts_with("/*!") {
                    // Block doc comment
                    let cleaned = self.clean_rust_block_comment(text);
                    comments.insert(0, cleaned);
                }
                break;
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if comments.is_empty() {
            None
        } else {
            Some(comments.join("\n"))
        }
    }

    /// Clean Rust block comment (/** ... */ or /*! ... */)
    fn clean_rust_block_comment(&self, raw: &str) -> String {
        let trimmed = raw.trim();

        // Remove /** or /*! and */
        let inner = if trimmed.starts_with("/**") || trimmed.starts_with("/*!") {
            &trimmed[3..trimmed.len() - 2]
        } else {
            trimmed
        };

        // Clean each line - remove leading * and whitespace
        inner
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('*') {
                    trimmed[1..].trim()
                } else {
                    trimmed
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    /// Build call graph for Rust code
    pub fn build_call_graph(&self, root: &Node, source: &str, file: &PathBuf) -> Vec<CallSite> {
        let mut call_sites = Vec::new();
        let mut current_function: Option<String> = None;
        self.visit_for_calls(root, source, &mut current_function, file, &mut call_sites);
        call_sites
    }

    fn visit_for_calls(
        &self,
        node: &Node,
        source: &str,
        current_function: &mut Option<String>,
        file: &PathBuf,
        call_sites: &mut Vec<CallSite>,
    ) {
        match node.kind() {
            "function_item" => {
                // Enter function context
                if let Some(name_node) = node.child_by_field_name("name") {
                    let func_name = &source[name_node.start_byte()..name_node.end_byte()];
                    let prev_function = current_function.clone();
                    *current_function = Some(func_name.to_string());

                    // Visit body
                    if let Some(body) = node.child_by_field_name("body") {
                        self.visit_for_calls(&body, source, current_function, file, call_sites);
                    }

                    // Restore previous function context
                    *current_function = prev_function;
                    return;
                }
            }
            "impl_item" => {
                // For impl blocks, visit methods
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        self.visit_for_calls(&child, source, current_function, file, call_sites);
                    }
                }
                return;
            }
            "call_expression" => {
                // Extract callee
                if let Some(func_node) = node.child_by_field_name("function") {
                    let callee_name = self.extract_rust_callee(&func_node, source);

                    if let Some(ref caller) = current_function {
                        call_sites.push(CallSite {
                            file: file.clone(),
                            span: Span {
                                start: node.start_byte(),
                                end: node.end_byte(),
                                start_line: node.start_position().row,
                                start_col: node.start_position().column,
                                end_line: node.end_position().row,
                                end_col: node.end_position().column,
                            },
                            callee: callee_name,
                            caller: caller.clone(),
                        });
                    }
                }
            }
            "method_call_expression" => {
                // Extract method name (last identifier before arguments)
                if let Some(name_node) = node.child_by_field_name("name") {
                    let method_name = &source[name_node.start_byte()..name_node.end_byte()];

                    if let Some(ref caller) = current_function {
                        call_sites.push(CallSite {
                            file: file.clone(),
                            span: Span {
                                start: node.start_byte(),
                                end: node.end_byte(),
                                start_line: node.start_position().row,
                                start_col: node.start_position().column,
                                end_line: node.end_position().row,
                                end_col: node.end_position().column,
                            },
                            callee: method_name.to_string(),
                            caller: caller.clone(),
                        });
                    }
                }
            }
            _ => {}
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_calls(&child, source, current_function, file, call_sites);
        }
    }

    /// Extract Rust callee name from call expression
    fn extract_rust_callee(&self, node: &Node, source: &str) -> String {
        match node.kind() {
            "identifier" => source[node.start_byte()..node.end_byte()].to_string(),
            "scoped_identifier" => {
                // For path::to::function, get just the function name
                if let Some(name) = node.child_by_field_name("name") {
                    source[name.start_byte()..name.end_byte()].to_string()
                } else {
                    source[node.start_byte()..node.end_byte()].to_string()
                }
            }
            "field_expression" => {
                // For obj.field, get the field name
                if let Some(field) = node.child_by_field_name("field") {
                    source[field.start_byte()..field.end_byte()].to_string()
                } else {
                    source[node.start_byte()..node.end_byte()].to_string()
                }
            }
            _ => source[node.start_byte()..node.end_byte()].to_string(),
        }
    }
}

impl Default for RustSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
