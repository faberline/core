use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::engine::{CallSite, SemanticSearchEngine};

impl SemanticSearchEngine {
    /// Build call graph from source code.
    pub fn build_call_graph(
        &mut self,
        file: PathBuf,
        content: &str,
        language: crate::syntax::Language,
    ) -> Result<(), String> {
        use crate::syntax::MultiParser;

        let mut parser =
            MultiParser::new().map_err(|e| format!("Failed to create parser: {:?}", e))?;
        let parsed = parser
            .parse(content, language)
            .ok_or("Failed to parse file")?;

        let call_sites =
            self.extract_call_sites(file.clone(), &parsed.tree.root_node(), content, language);

        for site in call_sites {
            self.call_graph.add_call(site);
        }

        Ok(())
    }

    /// Extract all call sites from an AST.
    fn extract_call_sites(
        &self,
        file: PathBuf,
        root: &tree_sitter::Node,
        source: &str,
        language: crate::syntax::Language,
    ) -> Vec<CallSite> {
        let mut call_sites = Vec::new();
        let mut current_function: Option<String> = None;

        self.visit_for_calls(
            root,
            source,
            language,
            &mut current_function,
            &file,
            &mut call_sites,
        );

        call_sites
    }

    /// Recursively visit AST nodes to find function calls.
    fn visit_for_calls(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        language: crate::syntax::Language,
        current_function: &mut Option<String>,
        file: &PathBuf,
        call_sites: &mut Vec<CallSite>,
    ) {
        match language {
            crate::syntax::Language::Python => {
                match node.kind() {
                    "function_definition" => {
                        // Extract function name
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let func_name = &source[name_node.start_byte()..name_node.end_byte()];
                            let prev_function = current_function.clone();
                            *current_function = Some(func_name.to_string());

                            // Visit children (function body)
                            let mut cursor = node.walk();
                            for child in node.children(&mut cursor) {
                                self.visit_for_calls(
                                    &child,
                                    source,
                                    language,
                                    current_function,
                                    file,
                                    call_sites,
                                );
                            }

                            // Restore previous function context
                            *current_function = prev_function;
                            return; // Don't visit children again
                        }
                    }
                    "call" => {
                        // Extract callee name
                        if let Some(func_node) = node.child_by_field_name("function") {
                            let callee_name = self.extract_function_name(&func_node, source);

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
                    _ => {}
                }
            }
            crate::syntax::Language::TypeScript => match node.kind() {
                "function_declaration" | "method_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let func_name = &source[name_node.start_byte()..name_node.end_byte()];
                        let prev_function = current_function.clone();
                        *current_function = Some(func_name.to_string());

                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            self.visit_for_calls(
                                &child,
                                source,
                                language,
                                current_function,
                                file,
                                call_sites,
                            );
                        }

                        *current_function = prev_function;
                        return;
                    }
                }
                "call_expression" => {
                    if let Some(func_node) = node.child_by_field_name("function") {
                        let callee_name = self.extract_function_name(&func_node, source);

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
                _ => {}
            },
            _ => {
                // Other languages not yet implemented
            }
        }

        // Visit children for all nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_calls(&child, source, language, current_function, file, call_sites);
        }
    }

    /// Extract function name from a node (handles attributes like obj.method).
    fn extract_function_name(&self, node: &tree_sitter::Node, source: &str) -> String {
        match node.kind() {
            "identifier" => source[node.start_byte()..node.end_byte()].to_string(),
            "attribute" => {
                // For obj.method, extract just "method"
                if let Some(attr_node) = node.child_by_field_name("attribute") {
                    source[attr_node.start_byte()..attr_node.end_byte()].to_string()
                } else {
                    source[node.start_byte()..node.end_byte()].to_string()
                }
            }
            "member_expression" => {
                // TypeScript: obj.method
                if let Some(property_node) = node.child_by_field_name("property") {
                    source[property_node.start_byte()..property_node.end_byte()].to_string()
                } else {
                    source[node.start_byte()..node.end_byte()].to_string()
                }
            }
            _ => {
                // Fallback: use full text
                source[node.start_byte()..node.end_byte()].to_string()
            }
        }
    }
}
