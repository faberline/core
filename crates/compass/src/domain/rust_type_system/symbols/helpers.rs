use tree_sitter::Node;

use crate::domain::rust_type_system::symbols::RustSymbolCollector;
use crate::domain::rust_type_system::types::{RustTypeParam, Visibility};
use crate::type_inference::TypeVarId;

impl RustSymbolCollector {
    // ========================================================================
    // Helper methods
    // ========================================================================

    pub(super) fn get_name(&self, node: &Node, source: &str) -> String {
        node.child_by_field_name("name")
            .map(|n| source[n.start_byte()..n.end_byte()].to_string())
            .unwrap_or_default()
    }

    pub(super) fn get_visibility(&self, node: &Node) -> Visibility {
        if let Some(vis) = node.child_by_field_name("visibility_modifier") {
            let mut cursor = vis.walk();
            for child in vis.children(&mut cursor) {
                match child.kind() {
                    "crate" => return Visibility::Crate,
                    "super" => return Visibility::Super,
                    _ => {}
                }
            }
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    pub(super) fn collect_type_params(&mut self, node: &Node, source: &str) -> Vec<RustTypeParam> {
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("type_parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                match child.kind() {
                    "type_identifier" => {
                        let name = source[child.start_byte()..child.end_byte()].to_string();
                        let id = TypeVarId(self.type_var_counter);
                        self.type_var_counter += 1;
                        params.push(RustTypeParam {
                            name,
                            id,
                            bounds: vec![],
                            default: None,
                        });
                    }
                    "constrained_type_parameter" => {
                        // Handle T: Bound syntax
                        if let Some(name_node) = child.child_by_field_name("left") {
                            let name =
                                source[name_node.start_byte()..name_node.end_byte()].to_string();
                            let id = TypeVarId(self.type_var_counter);
                            self.type_var_counter += 1;

                            let mut bounds = Vec::new();
                            if let Some(bounds_node) = child.child_by_field_name("bounds") {
                                bounds = self.parse_trait_bounds(&bounds_node, source);
                            }

                            params.push(RustTypeParam {
                                name,
                                id,
                                bounds,
                                default: None,
                            });
                        }
                    }
                    "optional_type_parameter" => {
                        // Handle T = Default syntax
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name =
                                source[name_node.start_byte()..name_node.end_byte()].to_string();
                            let id = TypeVarId(self.type_var_counter);
                            self.type_var_counter += 1;

                            let default = child
                                .child_by_field_name("default_type")
                                .map(|n| self.parse_type(&n, source));

                            params.push(RustTypeParam {
                                name,
                                id,
                                bounds: vec![],
                                default,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        params
    }
}
