use super::ScopeAnalyzer;
use crate::domain::semantic::scope::{ScopeKind, SymbolKind};
use crate::domain::syntax::parsed_file::{NodeRange, ParsedFile};

impl ScopeAnalyzer {
    pub(super) fn visit_node(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        match node.kind() {
            "function_definition" | "async_function_definition" => {
                // Define function name in current scope
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = file.node_text(&name_node).to_string();
                    self.current()
                        .define(name, SymbolKind::Function, name_node.to_range());
                }

                // Check if this is a stub function (body is just ... or pass)
                let is_stub = node
                    .child_by_field_name("body")
                    .map_or(false, |body| Self::is_stub_body(&body, file));

                // Create new scope for function body
                self.push_scope(ScopeKind::Function);

                // Process parameters (skip for stub functions)
                if !is_stub {
                    if let Some(params) = node.child_by_field_name("parameters") {
                        self.visit_parameters(&params, file);
                    }
                }

                // Visit body
                if let Some(body) = node.child_by_field_name("body") {
                    self.visit_children(&body, file);
                }

                self.pop_scope();
                return; // Don't visit children again
            }

            "class_definition" => {
                // Define class name in current scope
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = file.node_text(&name_node).to_string();
                    self.current()
                        .define(name, SymbolKind::Class, name_node.to_range());
                }

                // Create new scope for class body
                self.push_scope(ScopeKind::Class);

                // Visit body
                if let Some(body) = node.child_by_field_name("body") {
                    self.visit_children(&body, file);
                }

                self.pop_scope();
                return;
            }

            "lambda" => {
                self.push_scope(ScopeKind::Lambda);

                if let Some(params) = node.child_by_field_name("parameters") {
                    self.visit_parameters(&params, file);
                }

                if let Some(body) = node.child_by_field_name("body") {
                    self.visit_node(&body, file);
                }

                self.pop_scope();
                return;
            }

            "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression" => {
                self.push_scope(ScopeKind::Comprehension);
                self.visit_children(node, file);
                self.pop_scope();
                return;
            }

            "for_in_clause" => {
                // Loop variable in comprehension
                if let Some(left) = node.child_by_field_name("left") {
                    self.define_pattern(&left, file);
                }
            }

            "assignment" | "augmented_assignment" => {
                // Define variables on left side
                if let Some(left) = node.child_by_field_name("left") {
                    self.define_pattern(&left, file);
                }
                // Mark uses on right side
                if let Some(right) = node.child_by_field_name("right") {
                    self.visit_node(&right, file);
                }
                return;
            }

            "for_statement" => {
                // Loop variable
                if let Some(left) = node.child_by_field_name("left") {
                    self.define_pattern(&left, file);
                }
            }

            "except_clause" => {
                // Exception variable: except Exception as e
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "as_pattern" {
                        if let Some(alias) = child.child_by_field_name("alias") {
                            let name = file.node_text(&alias).to_string();
                            self.current()
                                .define(name, SymbolKind::Variable, alias.to_range());
                        }
                    }
                }
            }

            "with_item" => {
                // with open() as f
                if let Some(alias) = node.child_by_field_name("alias") {
                    self.define_pattern(&alias, file);
                }
            }

            "import_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        let name = file.node_text(&child);
                        let base_name = name.split('.').next().unwrap_or(name);
                        self.current().define(
                            base_name.to_string(),
                            SymbolKind::Import,
                            child.to_range(),
                        );
                    } else if child.kind() == "aliased_import" {
                        if let Some(alias) = child.child_by_field_name("alias") {
                            let name = file.node_text(&alias).to_string();
                            self.current()
                                .define(name, SymbolKind::Import, alias.to_range());
                        } else if let Some(name_node) = child.child_by_field_name("name") {
                            let name = file.node_text(&name_node).to_string();
                            self.current()
                                .define(name, SymbolKind::Import, name_node.to_range());
                        }
                    }
                }
                return;
            }

            "import_from_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        // Skip module name, we want imported names
                        continue;
                    }
                    if child.kind() == "aliased_import" {
                        if let Some(alias) = child.child_by_field_name("alias") {
                            let name = file.node_text(&alias).to_string();
                            self.current()
                                .define(name, SymbolKind::Import, alias.to_range());
                        } else if let Some(name_node) = child.child_by_field_name("name") {
                            let name = file.node_text(&name_node).to_string();
                            self.current()
                                .define(name, SymbolKind::Import, name_node.to_range());
                        }
                    }
                }
                return;
            }

            "global_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = file.node_text(&child).to_string();
                        self.current()
                            .define(name, SymbolKind::Global, child.to_range());
                    }
                }
                return;
            }

            "nonlocal_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = file.node_text(&child).to_string();
                        self.current()
                            .define(name, SymbolKind::Nonlocal, child.to_range());
                    }
                }
                return;
            }

            "identifier" => {
                // This is a use of an identifier
                let name = file.node_text(node);
                self.mark_used_in_scope(name);
                return;
            }

            _ => {}
        }

        // Visit children
        self.visit_children(node, file);
    }
}
