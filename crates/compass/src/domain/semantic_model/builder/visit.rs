use crate::domain::syntax::parsed_file::NodeRange;
use tree_sitter::Node;

use crate::domain::semantic_model::builder::SemanticModelBuilder;
use crate::domain::semantic_model::ids::SymbolId;
use crate::domain::semantic_model::symbol::{SemanticSymbolKind, SymbolData};
use crate::domain::semantic_model::type_info::TypeInfo;

impl<'a> SemanticModelBuilder<'a> {
    /// Visit a node and its children
    pub(super) fn visit_node(&mut self, node: &Node) {
        // Skip error nodes
        if node.is_error() || node.is_missing() {
            return;
        }

        match node.kind() {
            "function_definition" | "async_function_definition" => {
                self.visit_function(node);
                return;
            }
            "class_definition" => {
                self.visit_class(node);
                return;
            }
            "assignment" => {
                self.visit_assignment(node);
            }
            _ => {}
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(&child);
        }
    }

    /// Visit a function definition
    fn visit_function(&mut self, node: &Node) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();
        let def_range = name_node.as_ref().map(|n| n.to_range()).unwrap_or_default();

        // Get return type
        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| self.parse_type_from_node(&n))
            .unwrap_or(TypeInfo::Unknown);

        // Get parameters
        let params = self.collect_parameters(node);

        // Build callable type
        let type_info = TypeInfo::Callable {
            params,
            return_type: Box::new(return_type),
        };

        // Extract docstring
        let documentation = self.extract_docstring(node);

        // Add function symbol
        let symbol_id = self.model.add_symbol(SymbolData {
            name,
            kind: SemanticSymbolKind::Function,
            def_range: def_range.clone(),
            file_path: self.file_path.clone(),
            type_info: type_info.clone(),
            documentation,
            scope_id: self.current_scope,
            parent_id: None,
        });

        // Add typed range for the function name
        self.model
            .add_typed_range(def_range, type_info, Some(symbol_id));

        // Enter function scope
        let func_range = node.to_range();
        self.push_scope(func_range);

        // Process parameters
        if let Some(ref params) = node.child_by_field_name("parameters") {
            self.visit_parameters(params);
        }

        // Process body
        if let Some(ref body) = node.child_by_field_name("body") {
            self.visit_node(body);
        }

        self.pop_scope();
    }

    /// Visit a class definition
    fn visit_class(&mut self, node: &Node) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();
        let def_range = name_node.as_ref().map(|n| n.to_range()).unwrap_or_default();

        let type_info = TypeInfo::Instance {
            name: name.clone(),
            module: None,
            type_args: vec![],
        };

        let documentation = self.extract_docstring(node);

        let class_id = self.model.add_symbol(SymbolData {
            name,
            kind: SemanticSymbolKind::Class,
            def_range: def_range.clone(),
            file_path: self.file_path.clone(),
            type_info: type_info.clone(),
            documentation,
            scope_id: self.current_scope,
            parent_id: None,
        });

        self.model
            .add_typed_range(def_range, type_info, Some(class_id));

        // Enter class scope
        let class_range = node.to_range();
        self.push_scope(class_range);

        // Process body
        if let Some(ref body) = node.child_by_field_name("body") {
            self.visit_class_body(body, class_id);
        }

        self.pop_scope();
    }

    /// Visit class body and add methods/attributes
    fn visit_class_body(&mut self, body: &Node, class_id: SymbolId) {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "function_definition" | "async_function_definition" => {
                    self.visit_method(&child, class_id);
                }
                "expression_statement" => {
                    // Check for class attributes (type annotations)
                    if let Some(expr) = child.child(0) {
                        if expr.kind() == "assignment" || expr.kind() == "type" {
                            self.visit_class_attribute(&expr, class_id);
                        }
                    }
                }
                _ => self.visit_node(&child),
            }
        }
    }

    /// Visit a method (function inside class)
    fn visit_method(&mut self, node: &Node, class_id: SymbolId) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();
        let def_range = name_node.as_ref().map(|n| n.to_range()).unwrap_or_default();

        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| self.parse_type_from_node(&n))
            .unwrap_or(TypeInfo::Unknown);

        let params = self.collect_parameters(node);

        let type_info = TypeInfo::Callable {
            params,
            return_type: Box::new(return_type),
        };

        let documentation = self.extract_docstring(node);

        let method_id = self.model.add_symbol(SymbolData {
            name,
            kind: SemanticSymbolKind::Method,
            def_range: def_range.clone(),
            file_path: self.file_path.clone(),
            type_info: type_info.clone(),
            documentation,
            scope_id: self.current_scope,
            parent_id: Some(class_id),
        });

        self.model
            .add_typed_range(def_range, type_info, Some(method_id));

        // Process method body in its own scope
        let method_range = node.to_range();
        self.push_scope(method_range);

        if let Some(ref params) = node.child_by_field_name("parameters") {
            self.visit_parameters(params);
        }

        if let Some(ref body) = node.child_by_field_name("body") {
            self.visit_node(body);
        }

        self.pop_scope();
    }

    /// Visit a class attribute
    fn visit_class_attribute(&mut self, node: &Node, class_id: SymbolId) {
        if let Some(left) = node.child_by_field_name("left") {
            if left.kind() == "identifier" {
                let name = self.node_text(&left).to_string();
                let def_range = left.to_range();

                let type_info = node
                    .child_by_field_name("type")
                    .map(|n| self.parse_type_from_node(&n))
                    .unwrap_or(TypeInfo::Unknown);

                let symbol_id = self.model.add_symbol(SymbolData {
                    name,
                    kind: SemanticSymbolKind::Attribute,
                    def_range: def_range.clone(),
                    file_path: self.file_path.clone(),
                    type_info: type_info.clone(),
                    documentation: None,
                    scope_id: self.current_scope,
                    parent_id: Some(class_id),
                });

                self.model
                    .add_typed_range(def_range, type_info, Some(symbol_id));
            }
        }
    }

    /// Visit an assignment
    fn visit_assignment(&mut self, node: &Node) {
        if let Some(left) = node.child_by_field_name("left") {
            if left.kind() == "identifier" {
                let name = self.node_text(&left).to_string();
                let def_range = left.to_range();

                // Try to get type from annotation, or use Unknown
                let type_info = node
                    .child_by_field_name("type")
                    .map(|n| self.parse_type_from_node(&n))
                    .unwrap_or(TypeInfo::Unknown);

                let symbol_id = self.model.add_symbol(SymbolData {
                    name,
                    kind: SemanticSymbolKind::Variable,
                    def_range: def_range.clone(),
                    file_path: self.file_path.clone(),
                    type_info: type_info.clone(),
                    documentation: None,
                    scope_id: self.current_scope,
                    parent_id: None,
                });

                self.model
                    .add_typed_range(def_range, type_info, Some(symbol_id));
            }
        }

        // Visit right-hand side
        if let Some(ref right) = node.child_by_field_name("right") {
            self.visit_node(right);
        }
    }
}
