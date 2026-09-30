//! Rust symbol extraction visitor methods
//!
//! Implements R1-R7 from spec rust-symbol-analysis:
//! - R1: Extract Rust Functions (function_item)
//! - R2: Extract Rust Structs (struct_item)
//! - R3: Extract Rust Traits (trait_item)
//! - R4: Extract Rust Impls (impl_item)
//! - R5: Extract Rust Constants (const_item, static_item)
//! - R6: Extract Rust Doc Comments (///, //!)
//! - R7: Parse Rust Types into TypeInfo

#[cfg(test)]
mod tests;
mod type_defs;

use crate::domain::syntax::parsed_file::{NodeRange, ParsedFile};

use super::{SymbolKind, SymbolTableBuilder, TypeInfo};

impl SymbolTableBuilder {
    pub(crate) fn visit_rust_node(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if node.is_error() || node.is_missing() {
            return;
        }

        match node.kind() {
            "function_item" | "function_signature_item" => {
                self.visit_rust_function(node, file);
                return;
            }
            "struct_item" => {
                self.visit_rust_struct(node, file);
                return;
            }
            "enum_item" => {
                self.visit_rust_enum(node, file);
                return;
            }
            "trait_item" => {
                self.visit_rust_trait(node, file);
                return;
            }
            "impl_item" => {
                self.visit_rust_impl(node, file);
                return;
            }
            "const_item" => {
                self.visit_rust_const(node, file, SymbolKind::Const);
                return;
            }
            "static_item" => {
                self.visit_rust_const(node, file, SymbolKind::Static);
                return;
            }
            "type_item" => {
                self.visit_rust_type_alias(node, file);
                return;
            }
            "mod_item" => {
                self.visit_rust_mod(node, file);
                return;
            }
            "use_declaration" => {
                self.visit_rust_use(node, file);
                return;
            }
            "macro_definition" => {
                self.visit_rust_macro(node, file);
                return;
            }
            _ => {}
        }

        // Recurse children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_error() && !child.is_missing() {
                self.visit_rust_node(&child, file);
            }
        }
    }

    /// R1: Extract function definitions
    fn visit_rust_function(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        // R7: Parse return type
        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| TypeInfo::from_rust_type(file.node_text(&n)));

        // R6: Extract doc comments
        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Function,
            location,
            return_type,
            doc,
            self.current_scope,
        );

        // Enter function scope for parameters
        self.push_scope();

        if let Some(params) = node.child_by_field_name("parameters") {
            self.visit_rust_parameters(&params, file);
        }

        // Visit body for nested items
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_rust_node(&body, file);
        }

        self.pop_scope();
    }

    /// R4: Extract impl blocks
    fn visit_rust_impl(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        // Build impl name: "TypeName" or "TraitName for TypeName"
        let name = build_impl_name(node, file);
        let location = node
            .child_by_field_name("type")
            .map(|n| n.to_range())
            .unwrap_or_else(|| node.to_range());

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Impl,
            location,
            None,
            doc,
            self.current_scope,
        );

        // Enter impl scope for methods
        self.push_scope();

        if let Some(body) = node.child_by_field_name("body") {
            self.visit_rust_node(&body, file);
        }

        self.pop_scope();
    }

    /// R5: Extract const/static items
    fn visit_rust_const(
        &mut self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
        kind: SymbolKind,
    ) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let type_info = node
            .child_by_field_name("type")
            .map(|n| TypeInfo::from_rust_type(file.node_text(&n)));

        let doc = extract_rust_doc_comments(node, file);

        self.table
            .add_symbol(name, kind, location, type_info, doc, self.current_scope);
    }

    /// Extract type alias definitions
    fn visit_rust_type_alias(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::TypeAlias,
            location,
            None,
            doc,
            self.current_scope,
        );
    }

    /// Extract module declarations
    fn visit_rust_mod(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Module,
            location,
            None,
            doc,
            self.current_scope,
        );

        // If inline module (has body), enter scope and visit body
        if let Some(body) = node.child_by_field_name("body") {
            self.push_scope();
            self.visit_rust_node(&body, file);
            self.pop_scope();
        }
    }

    /// Extract use declarations
    fn visit_rust_use(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        // Extract the argument (use path)
        let arg = node.child_by_field_name("argument");
        if let Some(arg_node) = arg {
            let text = file.node_text(&arg_node).to_string();
            // Extract the last segment as the imported name
            let import_name = extract_use_name(&text);
            if !import_name.is_empty() {
                self.table.add_symbol(
                    import_name,
                    SymbolKind::Import,
                    arg_node.to_range(),
                    None,
                    None,
                    self.current_scope,
                );
            }
        }
    }

    /// Extract macro definitions
    fn visit_rust_macro(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| n.to_range()).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Macro,
            location,
            None,
            doc,
            self.current_scope,
        );
    }

    /// Extract function parameters
    fn visit_rust_parameters(&mut self, params: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            match child.kind() {
                "parameter" => {
                    // Regular parameter: pattern: type
                    let name_node = child.child_by_field_name("pattern");
                    let name = name_node
                        .map(|n| file.node_text(&n).to_string())
                        .unwrap_or_default();
                    let location = name_node.map(|n| n.to_range()).unwrap_or_default();

                    let type_info = child
                        .child_by_field_name("type")
                        .map(|n| TypeInfo::from_rust_type(file.node_text(&n)));

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
                "self_parameter" => {
                    self.table.add_symbol(
                        "self".to_string(),
                        SymbolKind::Parameter,
                        child.to_range(),
                        None,
                        None,
                        self.current_scope,
                    );
                }
                _ => {}
            }
        }
    }
}

/// R6: Extract doc comments from preceding siblings
fn extract_rust_doc_comments(node: &tree_sitter::Node<'_>, file: &ParsedFile) -> Option<String> {
    let mut doc_lines = Vec::new();
    let mut sibling = node.prev_sibling();

    // Walk backwards through preceding siblings collecting doc comments
    while let Some(sib) = sibling {
        let kind = sib.kind();
        if kind == "line_comment" {
            let text = file.node_text(&sib);
            if let Some(doc_text) = text.strip_prefix("///") {
                doc_lines.push(doc_text.strip_prefix(' ').unwrap_or(doc_text).to_string());
            } else if text.starts_with("//!") {
                // Inner doc comment — stop collecting outer docs
                break;
            } else {
                // Regular comment — stop
                break;
            }
        } else if kind == "attribute_item" || kind == "inner_attribute_item" {
            // Skip attributes between doc comments and item
            sibling = sib.prev_sibling();
            continue;
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

/// Build impl name from node: "TypeName" or "TraitName for TypeName"
fn build_impl_name(node: &tree_sitter::Node<'_>, file: &ParsedFile) -> String {
    let type_name = node
        .child_by_field_name("type")
        .map(|n| file.node_text(&n).to_string())
        .unwrap_or_default();

    let trait_name = node
        .child_by_field_name("trait")
        .map(|n| file.node_text(&n).to_string());

    match trait_name {
        Some(t) => format!("{} for {}", t, type_name),
        None => type_name,
    }
}

/// Extract the imported name from a use path
fn extract_use_name(path: &str) -> String {
    // Handle "as alias" rename
    if let Some((_, alias)) = path.rsplit_once(" as ") {
        return alias.trim().to_string();
    }
    // Handle glob imports
    if path.ends_with("::*") || path.contains('{') {
        return String::new();
    }
    // Take last segment
    path.rsplit("::").next().unwrap_or("").trim().to_string()
}
