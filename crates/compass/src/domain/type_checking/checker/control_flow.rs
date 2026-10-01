use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::narrowing::condition::negate_condition;
use crate::domain::narrowing::condition_parser::parse_condition;
use crate::domain::type_checking::checker::TypeChecker;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::ty::Type;

impl<'a> TypeChecker<'a> {
    /// Check if statement with type narrowing
    pub(super) fn check_if_statement(&mut self, node: &Node) {
        // Get the condition
        let condition = match node.child_by_field_name("condition") {
            Some(c) => c,
            None => return,
        };

        // Parse the condition into a narrowing condition
        let narrowing_cond = parse_condition(self.source, &condition);

        // Collect original types from environment for narrowing
        let original_types = self.inferencer.get_env_types();

        // Handle the consequence (if branch)
        if let Some(consequence) = node.child_by_field_name("consequence") {
            self.narrower.push_scope();
            self.narrower
                .apply_condition(&narrowing_cond, &original_types);

            // Set narrowed types as overrides in the inferencer
            let narrowed_types = self.collect_narrowed_types();
            self.inferencer.set_type_overrides(narrowed_types);

            // Check the body
            let mut cursor = consequence.walk();
            for child in consequence.children(&mut cursor) {
                self.check_node(&child);
            }

            self.inferencer.clear_type_overrides();
            self.narrower.pop_scope();
        }

        // Handle else/elif branches
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "else_clause" => {
                    self.narrower.push_scope();
                    // Apply negated condition for else branch
                    let negated = negate_condition(&narrowing_cond);
                    self.narrower.apply_condition(&negated, &original_types);

                    // Set narrowed types as overrides in the inferencer
                    let narrowed_types = self.collect_narrowed_types();
                    self.inferencer.set_type_overrides(narrowed_types);

                    // Check the else body
                    if let Some(body) = child.child_by_field_name("body") {
                        let mut body_cursor = body.walk();
                        for body_child in body.children(&mut body_cursor) {
                            self.check_node(&body_child);
                        }
                    }

                    self.inferencer.clear_type_overrides();
                    self.narrower.pop_scope();
                }
                "elif_clause" => {
                    // Recursively handle elif as another if
                    self.check_if_statement(&child);
                }
                _ => {}
            }
        }
    }

    /// Collect all narrowed types from the narrower
    fn collect_narrowed_types(&self) -> HashMap<String, Type> {
        let mut types = HashMap::new();
        // Get all narrowed types from the narrower scopes
        for name in self.inferencer.get_env_types().keys() {
            if let Some(ty) = self.narrower.get_narrowed(name) {
                types.insert(name.clone(), ty.clone());
            }
        }
        types
    }

    /// Check while statement with type narrowing
    pub(super) fn check_while_statement(&mut self, node: &Node) {
        // Get the condition
        let condition = match node.child_by_field_name("condition") {
            Some(c) => c,
            None => return,
        };

        // Parse the condition for narrowing
        let narrowing_cond = parse_condition(self.source, &condition);
        let original_types = self.inferencer.get_env_types();

        // Handle the body (condition is true inside loop)
        if let Some(body) = node.child_by_field_name("body") {
            self.narrower.push_scope();
            self.narrower
                .apply_condition(&narrowing_cond, &original_types);

            let narrowed_types = self.collect_narrowed_types();
            self.inferencer.set_type_overrides(narrowed_types);

            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.check_node(&child);
            }

            self.inferencer.clear_type_overrides();
            self.narrower.pop_scope();
        }

        // Handle else clause (executed when condition becomes false)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "else_clause" {
                if let Some(body) = child.child_by_field_name("body") {
                    let mut body_cursor = body.walk();
                    for body_child in body.children(&mut body_cursor) {
                        self.check_node(&body_child);
                    }
                }
            }
        }
    }

    /// Check for statement - binds loop variable
    pub(super) fn check_for_statement(&mut self, node: &Node) {
        // Get the iterable and infer its element type
        let iterable = node.child_by_field_name("right");
        let element_type = if let Some(iter_node) = iterable {
            let iter_type = self.inferencer.infer_expr(&iter_node);
            match iter_type {
                Type::List(elem) => (*elem).clone(),
                Type::Set(elem) => (*elem).clone(),
                Type::Dict(key, _) => (*key).clone(), // iterating dict gives keys
                Type::Tuple(elems) => {
                    if elems.is_empty() {
                        Type::Unknown
                    } else {
                        Type::union(elems)
                    }
                }
                Type::Str => Type::Str, // iterating str gives chars (single char strings)
                _ => Type::Unknown,
            }
        } else {
            Type::Unknown
        };

        // Bind the loop variable
        if let Some(target) = node.child_by_field_name("left") {
            self.inferencer.bind_assignment(&target, element_type);
        }

        // Check the body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.check_node(&child);
            }
        }

        // Handle else clause
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "else_clause" {
                if let Some(body) = child.child_by_field_name("body") {
                    let mut body_cursor = body.walk();
                    for body_child in body.children(&mut body_cursor) {
                        self.check_node(&body_child);
                    }
                }
            }
        }
    }

    /// Check try statement
    pub(super) fn check_try_statement(&mut self, node: &Node) {
        // Check the try body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.check_node(&child);
            }
        }

        // Check exception handlers
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "except_clause" => {
                    // Bind exception variable if present
                    if let Some(name) = child.child_by_field_name("name") {
                        // Exception type - default to BaseException if not specified
                        let exc_type = if let Some(type_node) = child.child_by_field_name("type") {
                            parse_type_annotation(self.source, &type_node)
                        } else {
                            Type::Instance {
                                name: "BaseException".to_string(),
                                module: Some("builtins".to_string()),
                                type_args: vec![],
                            }
                        };
                        self.inferencer.bind_assignment(&name, exc_type);
                    }

                    // Check except body
                    if let Some(body) = child.child_by_field_name("body") {
                        let mut body_cursor = body.walk();
                        for body_child in body.children(&mut body_cursor) {
                            self.check_node(&body_child);
                        }
                    }
                }
                "finally_clause" => {
                    if let Some(body) = child.child_by_field_name("body") {
                        let mut body_cursor = body.walk();
                        for body_child in body.children(&mut body_cursor) {
                            self.check_node(&body_child);
                        }
                    }
                }
                "else_clause" => {
                    if let Some(body) = child.child_by_field_name("body") {
                        let mut body_cursor = body.walk();
                        for body_child in body.children(&mut body_cursor) {
                            self.check_node(&body_child);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
