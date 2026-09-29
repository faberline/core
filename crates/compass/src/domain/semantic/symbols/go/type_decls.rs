use super::{extract_go_doc_comment, parse_go_type};
use crate::domain::diagnostic::model::Range;
use crate::domain::semantic::symbols::{SymbolKind, SymbolTableBuilder};
use crate::domain::syntax::parsed_file::ParsedFile;

impl SymbolTableBuilder {
    /// Extract type declarations (struct, interface, type alias)
    pub(super) fn visit_go_type_declaration(
        &mut self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                self.visit_go_type_spec(&child, file);
            }
        }
    }

    fn visit_go_type_spec(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

        let doc = extract_go_doc_comment(node, file).or_else(|| {
            // type_spec is inside type_declaration, try parent
            node.parent().and_then(|p| extract_go_doc_comment(&p, file))
        });

        // Determine kind from the type expression
        let type_node = node.child_by_field_name("type");
        let kind = match type_node.as_ref().map(|n| n.kind()) {
            Some("struct_type") => SymbolKind::Struct,
            Some("interface_type") => SymbolKind::Interface,
            _ => SymbolKind::Class, // type alias or other
        };

        self.table
            .add_symbol(name, kind, location, None, doc, self.current_scope);

        // Enter type scope for fields/methods
        if let Some(type_node) = type_node {
            self.push_scope();
            match type_node.kind() {
                "struct_type" => {
                    self.visit_go_struct_fields(&type_node, file);
                }
                "interface_type" => {
                    self.visit_go_interface_methods(&type_node, file);
                }
                _ => {}
            }
            self.pop_scope();
        }
    }

    fn visit_go_struct_fields(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        if let Some(field_list) = node.child_by_field_name("body") {
            let mut cursor = field_list.walk();
            for child in field_list.children(&mut cursor) {
                if child.kind() == "field_declaration" {
                    let name_node = child.child_by_field_name("name");
                    let name = name_node
                        .map(|n| file.node_text(&n).to_string())
                        .unwrap_or_default();
                    let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

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
    }

    fn visit_go_interface_methods(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "method_spec" {
                let name_node = child.child_by_field_name("name");
                let name = name_node
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

                if !name.is_empty() {
                    self.table.add_symbol(
                        name,
                        SymbolKind::Function,
                        location,
                        None,
                        None,
                        self.current_scope,
                    );
                }
            }
        }
    }
}
