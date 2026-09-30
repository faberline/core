//! Go symbol extraction visitor methods
//!
//! Extracts symbols from Go AST nodes:
//! - Package declarations (package_clause) → Module
//! - Functions (function_declaration) → Function
//! - Methods (method_declaration) → Function
//! - Types (type_declaration → type_spec) → Class/Struct/Interface
//! - Constants (const_declaration) → Const
//! - Variables (var_declaration) → Variable
//! - Imports (import_declaration) → Import

#[cfg(test)]
mod tests;
mod type_decls;

use crate::domain::syntax::parsed_file::{NodeRange, ParsedFile};

use super::{SymbolKind, SymbolTableBuilder, TypeInfo};

impl SymbolTableBuilder {
    pub(crate) fn visit_go_node(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if node.is_error() || node.is_missing() {
            return;
        }

        match node.kind() {
            "package_clause" => {
                self.visit_go_package(node, file);
                return;
            }
            "function_declaration" => {
                self.visit_go_function(node, file);
                return;
            }
            "method_declaration" => {
                self.visit_go_method(node, file);
                return;
            }
            "type_declaration" => {
                self.visit_go_type_declaration(node, file);
                return;
            }
            "const_declaration" => {
                self.visit_go_const_declaration(node, file);
                return;
            }
            "var_declaration" => {
                self.visit_go_var_declaration(node, file);
                return;
            }
            "import_declaration" => {
                self.visit_go_import_declaration(node, file);
                return;
            }
            _ => {}
        }

        // Recurse children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_error() && !child.is_missing() {
                self.visit_go_node(&child, file);
            }
        }
    }

    /// Extract package declaration
    fn visit_go_package(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "package_identifier" {
                let name = file.node_text(&child).to_string();
                let location = child.to_range();
                self.table.add_symbol(
                    name,
                    SymbolKind::Module,
                    location,
                    None,
                    None,
                    self.current_scope,
                );
                return;
            }
        }
    }

    /// Extract function declarations
    fn visit_go_function(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let return_type = self.extract_go_return_type(node, file);
        let doc = extract_go_doc_comment(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Function,
            location,
            return_type,
            doc,
            self.current_scope,
        );

        // Enter function scope for parameters and body
        self.push_scope();

        if let Some(params) = node.child_by_field_name("parameters") {
            self.visit_go_parameters(&params, file);
        }

        if let Some(body) = node.child_by_field_name("body") {
            self.visit_go_node(&body, file);
        }

        self.pop_scope();
    }

    /// Extract method declarations
    fn visit_go_method(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let return_type = self.extract_go_return_type(node, file);
        let doc = extract_go_doc_comment(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Function,
            location,
            return_type,
            doc,
            self.current_scope,
        );

        // Enter method scope
        self.push_scope();

        // Visit receiver as parameter
        if let Some(receiver) = node.child_by_field_name("receiver") {
            self.visit_go_parameters(&receiver, file);
        }

        if let Some(params) = node.child_by_field_name("parameters") {
            self.visit_go_parameters(&params, file);
        }

        if let Some(body) = node.child_by_field_name("body") {
            self.visit_go_node(&body, file);
        }

        self.pop_scope();
    }

    /// Extract const declarations
    fn visit_go_const_declaration(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "const_spec" {
                let name_node = child.child_by_field_name("name");
                let name = name_node
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                let location = name_node.map(|n| n.to_range()).unwrap_or_default();

                let type_info = child
                    .child_by_field_name("type")
                    .map(|n| parse_go_type(file.node_text(&n)));

                let doc = extract_go_doc_comment(&child, file)
                    .or_else(|| extract_go_doc_comment(node, file));

                if !name.is_empty() {
                    self.table.add_symbol(
                        name,
                        SymbolKind::Const,
                        location,
                        type_info,
                        doc,
                        self.current_scope,
                    );
                }
            }
        }
    }

    /// Extract var declarations
    fn visit_go_var_declaration(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "var_spec" {
                let name_node = child.child_by_field_name("name");
                let name = name_node
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                let location = name_node.map(|n| n.to_range()).unwrap_or_default();

                let type_info = child
                    .child_by_field_name("type")
                    .map(|n| parse_go_type(file.node_text(&n)));

                if !name.is_empty() {
                    self.table.add_symbol(
                        name,
                        SymbolKind::Variable,
                        location,
                        type_info,
                        None,
                        self.current_scope,
                    );
                }
            }
        }
    }

    /// Extract import declarations
    fn visit_go_import_declaration(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_spec" {
                let path_node = child.child_by_field_name("path");
                if let Some(path) = path_node {
                    let path_text = file.node_text(&path).trim_matches('"').to_string();
                    // Use alias if present, otherwise last segment of path
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| file.node_text(&n).to_string())
                        .unwrap_or_else(|| {
                            path_text
                                .rsplit('/')
                                .next()
                                .unwrap_or(&path_text)
                                .to_string()
                        });

                    if name != "_" && name != "." {
                        self.table.add_symbol(
                            name,
                            SymbolKind::Import,
                            path.to_range(),
                            None,
                            None,
                            self.current_scope,
                        );
                    }
                }
            }
        }
    }

    /// Extract function parameters
    fn visit_go_parameters(&mut self, params: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let name_node = child.child_by_field_name("name");
                let name = name_node
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                let location = name_node.map(|n| n.to_range()).unwrap_or_default();

                let type_info = child
                    .child_by_field_name("type")
                    .map(|n| parse_go_type(file.node_text(&n)));

                if !name.is_empty() {
                    self.table.add_symbol(
                        name,
                        SymbolKind::Parameter,
                        location,
                        type_info,
                        None,
                        self.current_scope,
                    );
                }
            }
        }
    }

    /// Extract return type from function/method result field
    fn extract_go_return_type(
        &self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
    ) -> Option<TypeInfo> {
        node.child_by_field_name("result")
            .map(|n| parse_go_type(file.node_text(&n)))
    }
}

/// Extract doc comment from preceding sibling (Go uses // comments)
fn extract_go_doc_comment(node: &tree_sitter::Node<'_>, file: &ParsedFile) -> Option<String> {
    let mut doc_lines = Vec::new();
    let mut sibling = node.prev_sibling();

    while let Some(sib) = sibling {
        if sib.kind() == "comment" {
            let text = file.node_text(&sib);
            if let Some(doc_text) = text.strip_prefix("//") {
                doc_lines.push(doc_text.strip_prefix(' ').unwrap_or(doc_text).to_string());
            } else {
                break;
            }
        } else {
            break;
        }
        sibling = sib.prev_sibling();
    }

    if doc_lines.is_empty() {
        return None;
    }

    // Reverse because we collected backwards
    doc_lines.reverse();
    Some(doc_lines.join("\n").trim().to_string())
}

/// Parse Go type string into TypeInfo
fn parse_go_type(type_str: &str) -> TypeInfo {
    let type_str = type_str.trim();

    if type_str.is_empty() {
        return TypeInfo::Unknown;
    }

    // Handle pointer types
    if let Some(inner) = type_str.strip_prefix('*') {
        return TypeInfo::Reference(Box::new(parse_go_type(inner)));
    }

    // Handle slice types
    if let Some(inner) = type_str.strip_prefix("[]") {
        return TypeInfo::List(Box::new(parse_go_type(inner)));
    }

    // Handle map types
    if type_str.starts_with("map[") {
        if let Some(bracket_end) = type_str.find(']') {
            let key = &type_str[4..bracket_end];
            let value = &type_str[bracket_end + 1..];
            return TypeInfo::Dict(Box::new(parse_go_type(key)), Box::new(parse_go_type(value)));
        }
    }

    // Handle Go primitives
    match type_str {
        "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32"
        | "uint64" | "uintptr" | "float32" | "float64" | "complex64" | "complex128" | "bool"
        | "byte" | "rune" | "string" => TypeInfo::Primitive(type_str.to_string()),
        "error" => TypeInfo::Named("error".to_string()),
        _ => TypeInfo::Named(type_str.to_string()),
    }
}
