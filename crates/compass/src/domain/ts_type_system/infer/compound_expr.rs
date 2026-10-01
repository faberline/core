use tree_sitter::Node;

use crate::domain::ts_type_system::infer::TsTypeInferencer;
use crate::type_inference::{Param, ParamKind, Type};

impl<'a> TsTypeInferencer<'a> {
    /// Infer object literal type
    pub(super) fn infer_object_literal(&mut self, node: &Node) -> Type {
        let mut members: Vec<(String, Type)> = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "pair" | "property" => {
                    let key = child.child_by_field_name("key");
                    let value = child.child_by_field_name("value");

                    if let (Some(k), Some(v)) = (key, value) {
                        let key_name = k
                            .utf8_text(self.source.as_bytes())
                            .unwrap_or("")
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string();
                        let value_ty = self.infer_expr(&v);
                        members.push((key_name, value_ty));
                    }
                }
                "shorthand_property_identifier" => {
                    let name = child.utf8_text(self.source.as_bytes()).unwrap_or("");
                    let ty = self
                        .context
                        .variables
                        .get(name)
                        .cloned()
                        .unwrap_or(Type::Unknown);
                    members.push((name.to_string(), ty));
                }
                "spread_element" => {
                    // Spread adds all properties from the spread target
                    if let Some(arg) = child.child(1) {
                        let spread_ty = self.infer_expr(&arg);
                        if let Type::Protocol {
                            members: spread_members,
                            ..
                        } = spread_ty
                        {
                            members.extend(spread_members);
                        }
                    }
                }
                _ => {}
            }
        }

        Type::Protocol {
            name: "".to_string(),
            module: None,
            members,
        }
    }

    /// Infer array literal type
    pub(super) fn infer_array_literal(&mut self, node: &Node) -> Type {
        let mut element_types = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "[" && child.kind() != "]" && child.kind() != "," {
                element_types.push(self.infer_expr(&child));
            }
        }

        if element_types.is_empty() {
            Type::list(Type::Unknown)
        } else {
            // Compute union of all element types
            let unified = if element_types.iter().all(|t| t == &element_types[0]) {
                element_types[0].clone()
            } else {
                Type::union(element_types)
            };
            Type::list(unified)
        }
    }

    /// Infer arrow function type
    pub(super) fn infer_arrow_function(&mut self, node: &Node) -> Type {
        let params = self.parse_function_params(node);

        // Infer return type from body
        let ret = if let Some(body) = node.child_by_field_name("body") {
            if body.kind() == "statement_block" {
                // Block body - would need to analyze return statements
                Type::Unknown
            } else {
                // Expression body
                self.infer_expr(&body)
            }
        } else {
            Type::Unknown
        };

        Type::Callable {
            params,
            ret: Box::new(ret),
        }
    }

    /// Infer function expression type
    pub(super) fn infer_function_expr(&mut self, node: &Node) -> Type {
        let params = self.parse_function_params(node);

        // Check for explicit return type annotation
        let ret = if let Some(return_type) = node.child_by_field_name("return_type") {
            self.parse_type_annotation(&return_type)
        } else {
            Type::Unknown
        };

        Type::Callable {
            params,
            ret: Box::new(ret),
        }
    }

    /// Parse function parameters
    fn parse_function_params(&mut self, node: &Node) -> Vec<Param> {
        let mut params = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                match child.kind() {
                    "required_parameter" | "optional_parameter" => {
                        let name = child
                            .child_by_field_name("pattern")
                            .or_else(|| child.child_by_field_name("name"))
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_default();

                        let ty = child
                            .child_by_field_name("type")
                            .map(|t| self.parse_type_annotation(&t))
                            .unwrap_or(Type::Unknown);

                        let has_default = child.child_by_field_name("value").is_some();
                        let optional = child.kind() == "optional_parameter";

                        params.push(Param {
                            name,
                            ty: if optional { Type::optional(ty) } else { ty },
                            has_default: has_default || optional,
                            kind: ParamKind::Positional,
                        });
                    }
                    "rest_parameter" => {
                        let name = child
                            .child_by_field_name("pattern")
                            .map(|n| self.node_text(&n).to_string())
                            .unwrap_or_default();

                        let ty = child
                            .child_by_field_name("type")
                            .map(|t| self.parse_type_annotation(&t))
                            .unwrap_or(Type::list(Type::Unknown));

                        params.push(Param {
                            name,
                            ty,
                            has_default: false,
                            kind: ParamKind::VarPositional,
                        });
                    }
                    _ => {}
                }
            }
        }

        params
    }

    /// Infer ternary expression type
    pub(super) fn infer_ternary_expr(&mut self, node: &Node) -> Type {
        let condition = node.child_by_field_name("condition");
        let consequence = node.child_by_field_name("consequence");
        let alternative = node.child_by_field_name("alternative");

        match (consequence, alternative) {
            (Some(c), Some(a)) => {
                // Apply narrowing based on condition
                if let Some(cond) = condition {
                    self.apply_type_guard(&cond, true);
                }
                let conseq_ty = self.infer_expr(&c);

                // Restore and apply opposite narrowing
                self.narrowed_types.clear();
                if let Some(cond) = condition {
                    self.apply_type_guard(&cond, false);
                }
                let alt_ty = self.infer_expr(&a);

                self.narrowed_types.clear();

                if conseq_ty == alt_ty {
                    conseq_ty
                } else {
                    Type::union(vec![conseq_ty, alt_ty])
                }
            }
            _ => Type::Unknown,
        }
    }

    /// Infer 'as' expression (type assertion)
    pub(super) fn infer_as_expr(&mut self, node: &Node) -> Type {
        if let Some(type_node) = node.child_by_field_name("type") {
            self.parse_type_annotation(&type_node)
        } else {
            Type::Unknown
        }
    }

    /// Infer type assertion (<Type>expr)
    pub(super) fn infer_type_assertion(&mut self, node: &Node) -> Type {
        if let Some(type_node) = node.child_by_field_name("type") {
            self.parse_type_annotation(&type_node)
        } else {
            Type::Unknown
        }
    }

    /// Infer new expression
    pub(super) fn infer_new_expr(&mut self, node: &Node) -> Type {
        let constructor = match node.child_by_field_name("constructor") {
            Some(c) => c,
            None => return Type::Unknown,
        };

        let constructor_name = constructor
            .utf8_text(self.source.as_bytes())
            .unwrap_or("")
            .to_string();

        // Parse type arguments if present (e.g., new Map<string, number>())
        let type_args = if let Some(type_args_node) = node.child_by_field_name("type_arguments") {
            let arg_nodes: Vec<Node> = {
                let mut cursor = type_args_node.walk();
                type_args_node
                    .children(&mut cursor)
                    .filter(|c| c.kind() != "<" && c.kind() != ">" && c.kind() != ",")
                    .collect()
            };
            arg_nodes
                .iter()
                .map(|c| self.parse_type_annotation(c))
                .collect()
        } else {
            vec![]
        };

        // Look up class
        if self.context.classes.contains_key(constructor_name.as_str()) {
            return Type::Instance {
                name: constructor_name,
                module: None,
                type_args,
            };
        }

        Type::Unknown
    }
}
