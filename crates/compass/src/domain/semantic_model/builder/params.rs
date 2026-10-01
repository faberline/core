use tree_sitter::Node;

use crate::diagnostic::Range;
use crate::domain::semantic_model::builder::SemanticModelBuilder;
use crate::domain::semantic_model::symbol::{SemanticSymbolKind, SymbolData};
use crate::domain::semantic_model::type_info::{ParamInfo, TypeInfo};

impl<'a> SemanticModelBuilder<'a> {
    /// Visit function parameters
    pub(super) fn visit_parameters(&mut self, params: &Node) {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    let name = self.node_text(&child).to_string();
                    let def_range = Range::from_node(&child);

                    let symbol_id = self.model.add_symbol(SymbolData {
                        name,
                        kind: SemanticSymbolKind::Parameter,
                        def_range: def_range.clone(),
                        file_path: self.file_path.clone(),
                        type_info: TypeInfo::Unknown,
                        documentation: None,
                        scope_id: self.current_scope,
                        parent_id: None,
                    });

                    self.model
                        .add_typed_range(def_range, TypeInfo::Unknown, Some(symbol_id));
                }
                "typed_parameter" | "typed_default_parameter" | "default_parameter" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = self.node_text(&name_node).to_string();
                        let def_range = Range::from_node(&name_node);

                        let type_info = child
                            .child_by_field_name("type")
                            .map(|n| self.parse_type_from_node(&n))
                            .unwrap_or(TypeInfo::Unknown);

                        let symbol_id = self.model.add_symbol(SymbolData {
                            name,
                            kind: SemanticSymbolKind::Parameter,
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
                _ => {}
            }
        }
    }

    /// Collect parameters as ParamInfo for type signatures
    pub(super) fn collect_parameters(&self, node: &Node) -> Vec<ParamInfo> {
        let mut params = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                let (name, type_info, has_default, is_variadic, is_keyword) = match child.kind() {
                    "identifier" => {
                        let name = self.node_text(&child).to_string();
                        (name, TypeInfo::Unknown, false, false, false)
                    }
                    "typed_parameter" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_default();
                        let ty = child
                            .child_by_field_name("type")
                            .map(|n| self.parse_type_from_node(&n))
                            .unwrap_or(TypeInfo::Unknown);
                        (name, ty, false, false, false)
                    }
                    "default_parameter" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_default();
                        (name, TypeInfo::Unknown, true, false, false)
                    }
                    "typed_default_parameter" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_default();
                        let ty = child
                            .child_by_field_name("type")
                            .map(|n| self.parse_type_from_node(&n))
                            .unwrap_or(TypeInfo::Unknown);
                        (name, ty, true, false, false)
                    }
                    "list_splat_pattern" => {
                        let name = child
                            .child(1)
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_else(|| "*args".to_string());
                        (name, TypeInfo::Unknown, false, true, false)
                    }
                    "dictionary_splat_pattern" => {
                        let name = child
                            .child(1)
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_else(|| "**kwargs".to_string());
                        (name, TypeInfo::Unknown, false, false, true)
                    }
                    _ => continue,
                };

                params.push(ParamInfo {
                    name,
                    type_info,
                    has_default,
                    is_variadic,
                    is_keyword,
                });
            }
        }

        params
    }

    /// Extract docstring from a function or class
    pub(super) fn extract_docstring(&self, node: &Node) -> Option<String> {
        let body = node.child_by_field_name("body")?;
        let mut cursor = body.walk();
        let first_child = body.children(&mut cursor).next()?;

        if first_child.kind() == "expression_statement" {
            if let Some(expr) = first_child.child(0) {
                if expr.kind() == "string" {
                    let text = self.node_text(&expr);
                    let doc = text
                        .trim_start_matches("\"\"\"")
                        .trim_start_matches("'''")
                        .trim_end_matches("\"\"\"")
                        .trim_end_matches("'''")
                        .trim();
                    return Some(doc.to_string());
                }
            }
        }
        None
    }
}
