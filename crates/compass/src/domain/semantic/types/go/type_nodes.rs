use super::{parse_primitive_or_named, ChannelDirection, GoType, GoTypeInference};
use crate::domain::syntax::parsed_file::ParsedFile;

impl GoTypeInference {
    pub(super) fn parse_type_node(
        &self,
        node: &tree_sitter::Node<'_>,
        file: &ParsedFile,
    ) -> GoType {
        match node.kind() {
            "struct_type" => self.parse_struct_type(node, file),
            "interface_type" => self.parse_interface_type(node, file),
            "channel_type" => self.parse_channel_type(node, file),
            "pointer_type" => {
                if let Some(inner) = node.child(1) {
                    GoType::Pointer(Box::new(self.parse_type_node(&inner, file)))
                } else {
                    GoType::Pointer(Box::new(GoType::Unknown))
                }
            }
            "slice_type" => {
                if let Some(elem) = node.child_by_field_name("element") {
                    GoType::Slice(Box::new(self.parse_type_node(&elem, file)))
                } else {
                    GoType::Slice(Box::new(GoType::Unknown))
                }
            }
            "map_type" => {
                let key = node
                    .child_by_field_name("key")
                    .map(|n| self.parse_type_node(&n, file))
                    .unwrap_or(GoType::Unknown);
                let value = node
                    .child_by_field_name("value")
                    .map(|n| self.parse_type_node(&n, file))
                    .unwrap_or(GoType::Unknown);
                GoType::Map(Box::new(key), Box::new(value))
            }
            "type_identifier" | "qualified_type" => {
                let text = file.node_text(node).to_string();
                parse_primitive_or_named(&text)
            }
            _ => {
                let text = file.node_text(node).trim().to_string();
                if text.is_empty() {
                    GoType::Unknown
                } else {
                    parse_primitive_or_named(&text)
                }
            }
        }
    }

    fn parse_struct_type(&self, node: &tree_sitter::Node<'_>, file: &ParsedFile) -> GoType {
        let mut fields = Vec::new();
        // struct_type -> field_declaration_list -> field_declaration*
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut inner = child.walk();
                for field_node in child.children(&mut inner) {
                    if field_node.kind() == "field_declaration" {
                        let fname = field_node
                            .child_by_field_name("name")
                            .map(|n| file.node_text(&n).to_string())
                            .unwrap_or_default();
                        let ftype = field_node
                            .child_by_field_name("type")
                            .map(|n| self.parse_type_node(&n, file))
                            .unwrap_or(GoType::Unknown);
                        if !fname.is_empty() {
                            fields.push((fname, ftype));
                        }
                    }
                }
                break;
            }
        }
        GoType::Struct { fields }
    }

    fn parse_interface_type(&self, node: &tree_sitter::Node<'_>, file: &ParsedFile) -> GoType {
        let mut methods = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // tree-sitter-go 0.25 uses "method_elem" for interface methods
            if child.kind() == "method_elem" || child.kind() == "method_spec" {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    methods.push(name);
                }
            }
        }
        GoType::Interface { methods }
    }

    fn parse_channel_type(&self, node: &tree_sitter::Node<'_>, file: &ParsedFile) -> GoType {
        let text = file.node_text(node);
        let direction = if text.starts_with("<-chan") {
            ChannelDirection::Receive
        } else if text.starts_with("chan<-") {
            ChannelDirection::Send
        } else {
            ChannelDirection::Bidirectional
        };

        // The element type is the last child (the value type after chan keyword)
        let element = node
            .child_by_field_name("value")
            .map(|n| self.parse_type_node(&n, file))
            .unwrap_or_else(|| {
                // Fallback: try last named child
                let count = node.named_child_count();
                if count > 0 {
                    node.named_child(count - 1)
                        .map(|n| self.parse_type_node(&n, file))
                        .unwrap_or(GoType::Unknown)
                } else {
                    GoType::Unknown
                }
            });

        GoType::Channel {
            element: Box::new(element),
            direction,
        }
    }
}
