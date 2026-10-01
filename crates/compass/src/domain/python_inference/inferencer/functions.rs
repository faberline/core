use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::ty::{Param, ParamKind, Type};

impl<'a> TypeInferencer<'a> {
    /// Analyze a function definition and add it to the environment
    pub fn analyze_function(&mut self, node: &Node) -> Type {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();

        let mut params = Vec::new();
        let mut return_type = Type::Unknown;

        // Parse parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            params = self.parse_parameters(&params_node);
        }

        // Parse return type annotation
        if let Some(return_node) = node.child_by_field_name("return_type") {
            return_type = parse_type_annotation(self.source, &return_node);
        }

        // Resolve Self type if we're in a class context
        if let Some(ref class_name) = self.current_class {
            return_type = self.resolve_self_type(return_type, class_name);
        }

        let func_type = Type::Callable {
            params,
            ret: Box::new(return_type),
        };

        // Check for @overload decorator
        if self.has_overload_decorator(node) {
            // This is an overload signature, collect it
            self.overload_signatures
                .entry(name.clone())
                .or_default()
                .push(func_type.clone());
            // Don't bind to env yet, wait for the implementation
            return func_type;
        }

        // Check if we have collected overload signatures for this function
        if let Some(signatures) = self.overload_signatures.remove(&name) {
            // Create an Overloaded type with all signatures
            let mut all_signatures = signatures;
            all_signatures.push(func_type.clone()); // Add the implementation signature
            let overloaded_type = Type::Overloaded {
                signatures: all_signatures,
            };
            self.env.bind(name, overloaded_type.clone());
            return overloaded_type;
        }

        self.env.bind(name, func_type.clone());
        func_type
    }

    /// Resolve Self type to the actual class type
    fn resolve_self_type(&self, ty: Type, class_name: &str) -> Type {
        match ty {
            Type::SelfType { .. } => Type::Instance {
                name: class_name.to_string(),
                module: None,
                type_args: vec![],
            },
            Type::Optional(inner) => {
                Type::Optional(Box::new(self.resolve_self_type(*inner, class_name)))
            }
            Type::Union(types) => Type::union(
                types
                    .into_iter()
                    .map(|t| self.resolve_self_type(t, class_name))
                    .collect(),
            ),
            Type::List(elem) => Type::List(Box::new(self.resolve_self_type(*elem, class_name))),
            other => other,
        }
    }

    /// Parse function parameters
    pub(super) fn parse_parameters(&mut self, node: &Node) -> Vec<Param> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(Param {
                        name: self.node_text(&child).to_string(),
                        ty: Type::Unknown,
                        has_default: false,
                        kind: ParamKind::Positional,
                    });
                }
                "typed_parameter" => {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| self.node_text(&n).to_string())
                        .unwrap_or_default();
                    let ty = child
                        .child_by_field_name("type")
                        .map(|t| parse_type_annotation(self.source, &t))
                        .unwrap_or(Type::Unknown);
                    params.push(Param {
                        name,
                        ty,
                        has_default: false,
                        kind: ParamKind::Positional,
                    });
                }
                "default_parameter" | "typed_default_parameter" => {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| self.node_text(&n).to_string())
                        .unwrap_or_default();
                    let ty = child
                        .child_by_field_name("type")
                        .map(|t| parse_type_annotation(self.source, &t))
                        .unwrap_or(Type::Unknown);
                    params.push(Param {
                        name,
                        ty,
                        has_default: true,
                        kind: ParamKind::Positional,
                    });
                }
                "list_splat_pattern" => {
                    if let Some(name_node) = child.child(1) {
                        params.push(Param {
                            name: self.node_text(&name_node).to_string(),
                            ty: Type::Tuple(vec![Type::Unknown]),
                            has_default: false,
                            kind: ParamKind::VarPositional,
                        });
                    }
                }
                "dictionary_splat_pattern" => {
                    if let Some(name_node) = child.child(1) {
                        params.push(Param {
                            name: self.node_text(&name_node).to_string(),
                            ty: Type::dict(Type::Str, Type::Unknown),
                            has_default: false,
                            kind: ParamKind::VarKeyword,
                        });
                    }
                }
                _ => {}
            }
        }

        params
    }
}
