use tree_sitter::Node;

use crate::domain::rust_type_system::symbols::RustSymbolCollector;
use crate::domain::rust_type_system::types::{EnumVariant, RustType, StructField, StructFields};

impl RustSymbolCollector {
    pub(super) fn collect_struct_fields(&self, node: &Node, source: &str) -> StructFields {
        if let Some(body) = node.child_by_field_name("body") {
            match body.kind() {
                "field_declaration_list" => {
                    let mut fields = Vec::new();
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        if child.kind() == "field_declaration" {
                            if let Some(field) = self.parse_field(&child, source) {
                                fields.push(field);
                            }
                        }
                    }
                    StructFields::Named(fields)
                }
                "ordered_field_declaration_list" => {
                    let mut types = Vec::new();
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        if !matches!(child.kind(), "(" | ")" | ",") {
                            types.push(self.parse_type(&child, source));
                        }
                    }
                    StructFields::Tuple(types)
                }
                _ => StructFields::Unit,
            }
        } else {
            StructFields::Unit
        }
    }

    fn parse_field(&self, node: &Node, source: &str) -> Option<StructField> {
        let name = node
            .child_by_field_name("name")
            .map(|n| source[n.start_byte()..n.end_byte()].to_string())?;
        let ty = node
            .child_by_field_name("type")
            .map(|n| self.parse_type(&n, source))
            .unwrap_or(RustType::Infer);
        let visibility = self.get_visibility(node);

        Some(StructField {
            name,
            ty,
            visibility,
        })
    }

    pub(super) fn collect_enum_variants(&self, node: &Node, source: &str) -> Vec<EnumVariant> {
        let mut variants = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_variant" {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                        .unwrap_or_default();
                    let fields = self.collect_variant_fields(&child, source);
                    let discriminant = self.parse_discriminant(&child, source);
                    variants.push(EnumVariant {
                        name,
                        fields,
                        discriminant,
                    });
                }
            }
        }
        variants
    }

    fn parse_discriminant(&self, node: &Node, source: &str) -> Option<i128> {
        // Look for the value field in enum_variant which contains the discriminant
        node.child_by_field_name("value").and_then(|value_node| {
            let text = source[value_node.start_byte()..value_node.end_byte()].trim();

            // Try to parse as integer (handles decimal, hex, octal, binary)
            if text.starts_with("0x") || text.starts_with("0X") {
                i128::from_str_radix(&text[2..].replace('_', ""), 16).ok()
            } else if text.starts_with("0o") || text.starts_with("0O") {
                i128::from_str_radix(&text[2..].replace('_', ""), 8).ok()
            } else if text.starts_with("0b") || text.starts_with("0B") {
                i128::from_str_radix(&text[2..].replace('_', ""), 2).ok()
            } else {
                text.replace('_', "").parse::<i128>().ok()
            }
        })
    }

    fn collect_variant_fields(&self, node: &Node, source: &str) -> StructFields {
        if let Some(body) = node.child_by_field_name("body") {
            self.collect_struct_fields_from_body(&body, source)
        } else {
            StructFields::Unit
        }
    }

    fn collect_struct_fields_from_body(&self, body: &Node, source: &str) -> StructFields {
        match body.kind() {
            "field_declaration_list" => {
                let mut fields = Vec::new();
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    if child.kind() == "field_declaration" {
                        if let Some(field) = self.parse_field(&child, source) {
                            fields.push(field);
                        }
                    }
                }
                StructFields::Named(fields)
            }
            "ordered_field_declaration_list" => {
                let mut types = Vec::new();
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    if !matches!(child.kind(), "(" | ")" | ",") {
                        types.push(self.parse_type(&child, source));
                    }
                }
                StructFields::Tuple(types)
            }
            _ => StructFields::Unit,
        }
    }
}
