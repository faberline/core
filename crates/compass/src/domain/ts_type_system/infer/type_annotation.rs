use tree_sitter::Node;

use crate::domain::ts_type_system::infer::TsTypeInferencer;
use crate::type_inference::{LiteralValue, Param, ParamKind, Type};

impl<'a> TsTypeInferencer<'a> {
    /// Parse type annotation from AST node
    pub fn parse_type_annotation(&mut self, node: &Node) -> Type {
        match node.kind() {
            // Predefined types
            "predefined_type" => {
                let text = self.node_text(node);
                match text {
                    "string" => Type::Str,
                    "number" => Type::Float,
                    "boolean" => Type::Bool,
                    "void" | "undefined" => Type::None,
                    "null" => Type::None,
                    "never" => Type::Never,
                    "any" => Type::Any,
                    "unknown" => Type::Unknown,
                    "object" => Type::Any, // Simplified
                    "symbol" => Type::Any, // Simplified
                    "bigint" => Type::Int,
                    _ => Type::Unknown,
                }
            }

            // Type identifier
            "type_identifier" | "identifier" => {
                let name = self.node_text(node);
                self.context.resolve_type(name).unwrap_or(Type::Unknown)
            }

            // Generic type: Array<T>, Promise<T>, etc.
            "generic_type" => self.parse_generic_type(node),

            // Union type: A | B
            "union_type" => {
                let mut cursor = node.walk();
                let types: Vec<Type> = node
                    .children(&mut cursor)
                    .filter(|c| c.kind() != "|")
                    .map(|c| self.parse_type_annotation(&c))
                    .collect();
                Type::union(types)
            }

            // Intersection type: A & B
            "intersection_type" => {
                let mut cursor = node.walk();
                let types: Vec<Type> = node
                    .children(&mut cursor)
                    .filter(|c| c.kind() != "&")
                    .map(|c| self.parse_type_annotation(&c))
                    .collect();
                Type::Intersection(types)
            }

            // Array type: T[]
            "array_type" => {
                if let Some(elem) = node.child(0) {
                    Type::list(self.parse_type_annotation(&elem))
                } else {
                    Type::list(Type::Unknown)
                }
            }

            // Tuple type: [A, B, C]
            "tuple_type" => {
                let mut cursor = node.walk();
                let types: Vec<Type> = node
                    .children(&mut cursor)
                    .filter(|c| c.kind() != "[" && c.kind() != "]" && c.kind() != ",")
                    .map(|c| self.parse_type_annotation(&c))
                    .collect();
                Type::Tuple(types)
            }

            // Function type: (a: A) => B
            "function_type" => self.parse_function_type(node),

            // Object type: { x: T, y: U }
            "object_type" => self.parse_object_type(node),

            // Literal type
            "literal_type" => {
                if let Some(child) = node.child(0) {
                    match child.kind() {
                        "string" => {
                            let text = self.node_text(&child);
                            let content = text.trim_matches(|c| c == '"' || c == '\'');
                            Type::Literal(LiteralValue::Str(content.to_string()))
                        }
                        "number" => {
                            let text = self.node_text(&child);
                            if let Ok(n) = text.parse::<i64>() {
                                Type::Literal(LiteralValue::Int(n))
                            } else if let Ok(n) = text.parse::<f64>() {
                                Type::Literal(LiteralValue::Float(n))
                            } else {
                                Type::Float
                            }
                        }
                        "true" => Type::Literal(LiteralValue::Bool(true)),
                        "false" => Type::Literal(LiteralValue::Bool(false)),
                        _ => Type::Unknown,
                    }
                } else {
                    Type::Unknown
                }
            }

            // Parenthesized type
            "parenthesized_type" => {
                if let Some(inner) = node.child(1) {
                    self.parse_type_annotation(&inner)
                } else {
                    Type::Unknown
                }
            }

            // Readonly type
            "readonly_type" => {
                if let Some(inner) = node.child(1) {
                    self.parse_type_annotation(&inner) // Just ignore readonly
                } else {
                    Type::Unknown
                }
            }

            // Conditional type: T extends U ? X : Y
            "conditional_type" => self.parse_conditional_type(node),

            // Indexed access type: T[K]
            "indexed_access_type" => self.parse_indexed_access_type(node),

            // Mapped type: { [K in keyof T]: V }
            "mapped_type_clause" | "mapped_type" => self.parse_mapped_type(node),

            // Template literal type: `hello ${string}`
            "template_literal_type" => self.parse_template_literal_type(node),

            // keyof type
            "keyof_type" | "type_query" => {
                // Simplified: return string | number | symbol
                Type::Union(vec![Type::Str, Type::Int])
            }

            _ => Type::Unknown,
        }
    }

    /// Parse generic type like Array<T> or Map<K, V>
    fn parse_generic_type(&mut self, node: &Node) -> Type {
        let name_node = node.child_by_field_name("name");
        let args_node = node.child_by_field_name("type_arguments");

        // Extract name as owned string first
        let name = name_node
            .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            .unwrap_or("")
            .to_string();

        // Collect child nodes first, then parse
        let type_arg_nodes: Vec<Node> = args_node
            .map(|args| {
                let mut cursor = args.walk();
                args.children(&mut cursor)
                    .filter(|c| c.kind() != "<" && c.kind() != ">" && c.kind() != ",")
                    .collect()
            })
            .unwrap_or_default();

        let type_args: Vec<Type> = type_arg_nodes
            .iter()
            .map(|c| self.parse_type_annotation(c))
            .collect();

        // Handle built-in generics
        match name.as_str() {
            "Array" | "ReadonlyArray" => {
                if let Some(elem) = type_args.into_iter().next() {
                    Type::list(elem)
                } else {
                    Type::list(Type::Unknown)
                }
            }
            "Promise" | "PromiseLike" => {
                if let Some(elem) = type_args.into_iter().next() {
                    // Return Promise<T> as-is (could create a wrapper type)
                    Type::Instance {
                        name: "Promise".to_string(),
                        module: None,
                        type_args: vec![elem],
                    }
                } else {
                    Type::Instance {
                        name: "Promise".to_string(),
                        module: None,
                        type_args: vec![Type::Unknown],
                    }
                }
            }
            "Map" | "ReadonlyMap" => {
                if type_args.len() >= 2 {
                    Type::dict(type_args[0].clone(), type_args[1].clone())
                } else {
                    Type::dict(Type::Unknown, Type::Unknown)
                }
            }
            "Set" | "ReadonlySet" => {
                if let Some(elem) = type_args.into_iter().next() {
                    Type::Set(Box::new(elem))
                } else {
                    Type::Set(Box::new(Type::Unknown))
                }
            }
            "Partial" => {
                // Make all properties optional
                if let Some(inner) = type_args.into_iter().next() {
                    self.make_partial(inner)
                } else {
                    Type::Unknown
                }
            }
            "Required" => {
                // Make all properties required
                if let Some(inner) = type_args.into_iter().next() {
                    self.make_required(inner)
                } else {
                    Type::Unknown
                }
            }
            "Readonly" => {
                // Just return inner type (readonly not tracked)
                type_args.into_iter().next().unwrap_or(Type::Unknown)
            }
            "Record" => {
                if type_args.len() >= 2 {
                    Type::dict(type_args[0].clone(), type_args[1].clone())
                } else {
                    Type::dict(Type::Str, Type::Unknown)
                }
            }
            _ => {
                // User-defined generic type
                Type::Instance {
                    name: name.to_string(),
                    module: None,
                    type_args,
                }
            }
        }
    }

    /// Parse function type: (params) => ReturnType
    fn parse_function_type(&mut self, node: &Node) -> Type {
        let params_node = node.child_by_field_name("parameters");
        let return_node = node.child_by_field_name("return_type");

        let params: Vec<Param> = params_node
            .map(|pn| {
                let mut cursor = pn.walk();
                pn.children(&mut cursor)
                    .filter_map(|c| {
                        if c.kind() == "parameter" || c.kind() == "required_parameter" {
                            let name = c
                                .child_by_field_name("name")
                                .map(|n| self.node_text(&n).to_string())
                                .unwrap_or_default();
                            let ty = c
                                .child_by_field_name("type")
                                .map(|t| self.parse_type_annotation(&t))
                                .unwrap_or(Type::Unknown);
                            Some(Param {
                                name,
                                ty,
                                has_default: false,
                                kind: ParamKind::Positional,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let ret = return_node
            .map(|r| self.parse_type_annotation(&r))
            .unwrap_or(Type::Unknown);

        Type::Callable {
            params,
            ret: Box::new(ret),
        }
    }

    /// Parse object type: { x: T, y: U }
    fn parse_object_type(&mut self, node: &Node) -> Type {
        let mut members: Vec<(String, Type)> = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "property_signature" {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| self.node_text(&n).to_string())
                    .unwrap_or_default();
                let ty = child
                    .child_by_field_name("type")
                    .map(|t| self.parse_type_annotation(&t))
                    .unwrap_or(Type::Unknown);
                members.push((name, ty));
            } else if child.kind() == "method_signature" {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| self.node_text(&n).to_string())
                    .unwrap_or_default();
                let ty = self.parse_function_type(&child);
                members.push((name, ty));
            }
        }

        Type::Protocol {
            name: "".to_string(),
            module: None,
            members,
        }
    }
}
