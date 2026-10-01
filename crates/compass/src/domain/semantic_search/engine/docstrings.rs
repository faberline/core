use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::syntax::parsed_file::ParsedFile;

impl SemanticSearchEngine {
    /// Extract the docstrings of a parsed file.
    /// Returns a map of symbol name -> docstring.
    ///
    /// `extract_docstrings` (in the composition root) parses the source first.
    pub fn extract_docstrings_parsed(
        &self,
        parsed: &ParsedFile,
    ) -> std::collections::HashMap<String, String> {
        self.extract_docstrings_from_ast(&parsed.tree.root_node(), &parsed.source, parsed.language)
    }

    /// Extract docstrings from AST nodes.
    fn extract_docstrings_from_ast(
        &self,
        root: &tree_sitter::Node,
        source: &str,
        language: crate::syntax::Language,
    ) -> std::collections::HashMap<String, String> {
        let mut docstrings = std::collections::HashMap::new();

        self.visit_for_docstrings(root, source, language, &mut docstrings);

        docstrings
    }

    /// Recursively visit AST nodes to extract docstrings.
    fn visit_for_docstrings(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        language: crate::syntax::Language,
        docstrings: &mut std::collections::HashMap<String, String>,
    ) {
        match language {
            crate::syntax::Language::Python => {
                match node.kind() {
                    "function_definition" | "class_definition" => {
                        // Extract function/class name
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let symbol_name = &source[name_node.start_byte()..name_node.end_byte()];

                            // Look for docstring (first string in body)
                            if let Some(body_node) = node.child_by_field_name("body") {
                                let docstring = self.extract_python_docstring(&body_node, source);
                                if let Some(doc) = docstring {
                                    docstrings.insert(symbol_name.to_string(), doc);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            crate::syntax::Language::TypeScript => {
                match node.kind() {
                    "function_declaration" | "method_definition" | "class_declaration" => {
                        // Extract name
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let symbol_name = &source[name_node.start_byte()..name_node.end_byte()];

                            // Look for JSDoc comment before this node
                            if let Some(prev_sibling) = node.prev_sibling() {
                                if prev_sibling.kind() == "comment" {
                                    let comment_text =
                                        &source[prev_sibling.start_byte()..prev_sibling.end_byte()];
                                    if comment_text.starts_with("/**") {
                                        // JSDoc comment
                                        let cleaned = self.clean_jsdoc(comment_text);
                                        docstrings.insert(symbol_name.to_string(), cleaned);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            crate::syntax::Language::Rust => {
                // Rust doc comments: `/// …` lines immediately before item definitions.
                // tree-sitter-rust models doc comments as `line_comment` nodes with text
                // starting with `///`. We look for a function/struct/enum/trait item whose
                // immediately-preceding siblings are all `///` line_comment nodes.
                let item_kinds = [
                    "function_item",
                    "struct_item",
                    "enum_item",
                    "trait_item",
                    "impl_item",
                    "type_item",
                ];
                if item_kinds.contains(&node.kind()) {
                    // Extract name
                    let name_opt = node
                        .child_by_field_name("name")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string());

                    if let Some(symbol_name) = name_opt {
                        // Walk backwards through previous siblings collecting `/// ` comments
                        let mut doc_lines: Vec<String> = Vec::new();
                        let mut sib = node.prev_sibling();
                        while let Some(prev) = sib {
                            if prev.kind() == "line_comment" {
                                let text = &source[prev.start_byte()..prev.end_byte()];
                                let trimmed = text.trim_start_matches('/').trim();
                                doc_lines.push(trimmed.to_string());
                                sib = prev.prev_sibling();
                            } else if prev.kind() == "attribute_item" {
                                // Skip attributes like `#[derive(...)]`
                                sib = prev.prev_sibling();
                            } else {
                                break;
                            }
                        }
                        if !doc_lines.is_empty() {
                            doc_lines.reverse();
                            docstrings.insert(symbol_name, doc_lines.join("\n"));
                        }
                    }
                }
            }
            _ => {
                // Other languages not yet implemented
            }
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_for_docstrings(&child, source, language, docstrings);
        }
    }

    /// Extract Python docstring from function/class body.
    fn extract_python_docstring(
        &self,
        body_node: &tree_sitter::Node,
        source: &str,
    ) -> Option<String> {
        // Python docstring is the first expression_statement containing a string
        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                // Check if it contains a string
                let mut expr_cursor = child.walk();
                for expr_child in child.children(&mut expr_cursor) {
                    if expr_child.kind() == "string" {
                        let string_content =
                            &source[expr_child.start_byte()..expr_child.end_byte()];
                        // Remove quotes and clean up
                        let cleaned = self.clean_python_docstring(string_content);
                        return Some(cleaned);
                    }
                }
            } else if child.kind() != "comment" {
                // First non-comment, non-string statement - no docstring
                break;
            }
        }
        None
    }

    /// Clean Python docstring (remove quotes, trim).
    fn clean_python_docstring(&self, raw: &str) -> String {
        let trimmed = raw.trim();

        // Remove triple quotes or single quotes
        let cleaned = if trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\"") {
            &trimmed[3..trimmed.len() - 3]
        } else if trimmed.starts_with("'''") && trimmed.ends_with("'''") {
            &trimmed[3..trimmed.len() - 3]
        } else if trimmed.starts_with('"') && trimmed.ends_with('"') {
            &trimmed[1..trimmed.len() - 1]
        } else if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };

        cleaned.trim().to_string()
    }

    /// Clean JSDoc comment (remove /** */ and leading *).
    fn clean_jsdoc(&self, raw: &str) -> String {
        let trimmed = raw.trim();

        // Remove /** and */
        let content = if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
            &trimmed[3..trimmed.len() - 2]
        } else if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            &trimmed[2..trimmed.len() - 2]
        } else {
            trimmed
        };

        // Remove leading * from each line
        content
            .lines()
            .map(|line| {
                let l = line.trim();
                if l.starts_with('*') {
                    l[1..].trim()
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }
}
