use super::extract_rust_doc_comments;
use crate::domain::diagnostic::model::Range;
use crate::domain::semantic::symbols::{SymbolKind, SymbolTableBuilder, TypeInfo};
use crate::domain::syntax::parsed_file::ParsedFile;

impl SymbolTableBuilder {
    /// R2: Extract struct definitions
    pub(super) fn visit_rust_struct(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Struct,
            location,
            None,
            doc,
            self.current_scope,
        );

        // Enter struct scope for fields
        self.push_scope();

        if let Some(body) = node.child_by_field_name("body") {
            self.visit_rust_struct_fields(&body, file);
        }

        self.pop_scope();
    }

    fn visit_rust_struct_fields(&mut self, body: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                let name_node = child.child_by_field_name("name");
                let name = name_node
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

                let type_info = child
                    .child_by_field_name("type")
                    .map(|n| TypeInfo::from_rust_type(file.node_text(&n)));

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

    /// Extract enum definitions
    pub(super) fn visit_rust_enum(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Enum,
            location,
            None,
            doc,
            self.current_scope,
        );

        // Enter enum scope for variants
        self.push_scope();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_variant" {
                    let vname_node = child.child_by_field_name("name");
                    let vname = vname_node
                        .map(|n| file.node_text(&n).to_string())
                        .unwrap_or_default();
                    let vlocation = vname_node.map(|n| Range::from_node(&n)).unwrap_or_default();

                    self.table.add_symbol(
                        vname,
                        SymbolKind::EnumMember,
                        vlocation,
                        None,
                        None,
                        self.current_scope,
                    );
                }
            }
        }

        self.pop_scope();
    }

    /// R3: Extract trait definitions
    pub(super) fn visit_rust_trait(&mut self, node: &tree_sitter::Node<'_>, file: &ParsedFile) {
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| file.node_text(&n).to_string())
            .unwrap_or_default();
        let location = name_node.map(|n| Range::from_node(&n)).unwrap_or_default();

        let doc = extract_rust_doc_comments(node, file);

        self.table.add_symbol(
            name,
            SymbolKind::Trait,
            location,
            None,
            doc,
            self.current_scope,
        );

        // Enter trait scope for methods
        self.push_scope();

        if let Some(body) = node.child_by_field_name("body") {
            self.visit_rust_node(&body, file);
        }

        self.pop_scope();
    }
}
