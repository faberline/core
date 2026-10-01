use tree_sitter::Node;

use crate::domain::ts_type_system::infer::TsTypeInferencer;
use crate::domain::ts_type_system::types::is_assignable_to;
use crate::type_inference::{LiteralValue, Type};

impl<'a> TsTypeInferencer<'a> {
    /// Parse conditional type: T extends U ? X : Y
    ///
    /// When T is known, we can evaluate the condition immediately.
    /// When T is a type variable or unknown, we store the full conditional
    /// as a union of both branches (conservative approximation).
    pub(super) fn parse_conditional_type(&mut self, node: &Node) -> Type {
        let check = node.child_by_field_name("left");
        let extends = node.child_by_field_name("right");
        let consequence = node.child_by_field_name("consequence");
        let alternative = node.child_by_field_name("alternative");

        let check_ty = check
            .map(|c| self.parse_type_annotation(&c))
            .unwrap_or(Type::Unknown);
        let extends_ty = extends
            .map(|e| self.parse_type_annotation(&e))
            .unwrap_or(Type::Unknown);

        // If check type is a type variable or unknown, we cannot resolve the
        // conditional at this point. Return a union of both branches as a
        // conservative approximation.
        let check_is_unresolved = matches!(&check_ty, Type::TypeVar { .. } | Type::Unknown);

        if check_is_unresolved {
            let true_ty = consequence
                .map(|c| self.parse_type_annotation(&c))
                .unwrap_or(Type::Unknown);
            let false_ty = alternative
                .map(|a| self.parse_type_annotation(&a))
                .unwrap_or(Type::Unknown);
            Type::union(vec![true_ty, false_ty])
        } else if is_assignable_to(&check_ty, &extends_ty) {
            consequence
                .map(|c| self.parse_type_annotation(&c))
                .unwrap_or(Type::Unknown)
        } else {
            alternative
                .map(|a| self.parse_type_annotation(&a))
                .unwrap_or(Type::Unknown)
        }
    }

    /// Parse indexed access type: T[K]
    pub(super) fn parse_indexed_access_type(&mut self, node: &Node) -> Type {
        let object = node.child_by_field_name("object");
        let index = node.child_by_field_name("index");

        let object_ty = object
            .map(|o| self.parse_type_annotation(&o))
            .unwrap_or(Type::Unknown);
        let index_ty = index
            .map(|i| self.parse_type_annotation(&i))
            .unwrap_or(Type::Unknown);

        // If index is a literal string, look up property
        if let Type::Literal(LiteralValue::Str(key)) = &index_ty {
            return self.get_property_type(&object_ty, key);
        }

        // Otherwise return union of all values (simplified)
        Type::Unknown
    }

    /// Parse mapped type: { [K in keyof T]: V }
    ///
    /// Mapped types iterate over keys from a source type and produce
    /// a new object type. When the source keys are known (e.g., a union
    /// of literal strings), we can expand the mapped type into a concrete
    /// Protocol. Otherwise, we represent it as a Dict.
    pub(super) fn parse_mapped_type(&mut self, node: &Node) -> Type {
        // Look for the type parameter, constraint, and value type.
        // tree-sitter-typescript represents mapped types with children:
        //   { [ name "in" constraint ] : value_type }
        let mut key_name = String::new();
        let mut constraint_ty = Type::Unknown;
        let mut value_ty = Type::Unknown;

        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        // Walk children to find the mapping clause and value type.
        // Pattern: "{" "[" identifier "in" type "]" ":" type "}"
        // Or the mapped_type_clause child contains (name, constraint).
        for child in &children {
            match child.kind() {
                "mapped_type_clause" => {
                    // The clause has name and constraint children
                    if let Some(name_node) = child.child_by_field_name("name") {
                        key_name = name_node
                            .utf8_text(self.source.as_bytes())
                            .unwrap_or("")
                            .to_string();
                    }
                    if let Some(type_node) = child.child_by_field_name("type") {
                        constraint_ty = self.parse_type_annotation(&type_node);
                    }
                }
                "type_annotation" | _
                    if child.kind().contains("type")
                        && child.kind() != "mapped_type_clause"
                        && !matches!(child.kind(), "{" | "}" | "[" | "]" | ":" | "in") =>
                {
                    // This is the value type
                    value_ty = self.parse_type_annotation(child);
                }
                _ => {}
            }
        }

        // If constraint is a union of literal strings, expand to Protocol
        if let Type::Union(members) = &constraint_ty {
            let all_literal_strings = members
                .iter()
                .all(|m| matches!(m, Type::Literal(LiteralValue::Str(_))));

            if all_literal_strings {
                let protocol_members: Vec<(String, Type)> = members
                    .iter()
                    .filter_map(|m| {
                        if let Type::Literal(LiteralValue::Str(s)) = m {
                            Some((s.clone(), value_ty.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                return Type::Protocol {
                    name: String::new(),
                    module: None,
                    members: protocol_members,
                };
            }
        }

        // Fallback: represent as a dictionary from the key constraint to the value
        let _ = key_name; // Consumed for expansion above, not needed for Dict
        Type::dict(constraint_ty, value_ty)
    }

    /// Parse template literal type: `hello ${string}`
    ///
    /// Template literal types are string types built from static segments
    /// and interpolated type positions. When all interpolations resolve to
    /// concrete literal strings, we can produce a literal string type.
    /// Otherwise we return Type::Str.
    pub(super) fn parse_template_literal_type(&mut self, node: &Node) -> Type {
        let mut all_literal = true;
        let mut result = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                // Static text segments
                "template_type_string" | "string_fragment" | "template_chars" => {
                    let text = child.utf8_text(self.source.as_bytes()).unwrap_or("");
                    result.push_str(text);
                }
                // Interpolation: ${Type}
                "template_type" => {
                    // The interpolated type is the first child that is a type node
                    if let Some(type_child) = child.child(0) {
                        let inner_ty = self.parse_type_annotation(&type_child);
                        match &inner_ty {
                            Type::Literal(LiteralValue::Str(s)) => {
                                result.push_str(s);
                            }
                            _ => {
                                all_literal = false;
                            }
                        }
                    } else {
                        all_literal = false;
                    }
                }
                // Skip delimiters
                "`" | "${" | "}" => {}
                _ => {
                    // Any child that is a type node inside an interpolation
                    let text = child.utf8_text(self.source.as_bytes()).unwrap_or("");
                    if !text.is_empty() && !matches!(child.kind(), "{" | "}" | "`" | "${") {
                        // Try to parse as type for interpolation
                        let inner_ty = self.parse_type_annotation(&child);
                        match &inner_ty {
                            Type::Literal(LiteralValue::Str(s)) => {
                                result.push_str(s);
                            }
                            _ => {
                                all_literal = false;
                            }
                        }
                    }
                }
            }
        }

        if all_literal && !result.is_empty() {
            Type::Literal(LiteralValue::Str(result))
        } else {
            Type::Str
        }
    }

    /// Unwrap Promise<T> to T
    pub(super) fn unwrap_promise(&self, ty: Type) -> Type {
        match ty {
            Type::Instance {
                name, type_args, ..
            } if name == "Promise" => type_args.into_iter().next().unwrap_or(Type::Unknown),
            _ => ty,
        }
    }

    /// Make a type partial (all properties optional)
    pub(super) fn make_partial(&self, ty: Type) -> Type {
        match ty {
            Type::Protocol {
                name,
                module,
                members,
            } => {
                let new_members: Vec<(String, Type)> = members
                    .into_iter()
                    .map(|(k, v)| (k, Type::optional(v)))
                    .collect();
                Type::Protocol {
                    name,
                    module,
                    members: new_members,
                }
            }
            other => Type::optional(other),
        }
    }

    /// Make a type required (all properties required)
    pub(super) fn make_required(&self, ty: Type) -> Type {
        match ty {
            Type::Protocol {
                name,
                module,
                members,
            } => {
                let new_members: Vec<(String, Type)> = members
                    .into_iter()
                    .map(|(k, v)| {
                        let unwrapped = match v {
                            Type::Optional(inner) => (*inner).clone(),
                            other => other,
                        };
                        (k, unwrapped)
                    })
                    .collect();
                Type::Protocol {
                    name,
                    module,
                    members: new_members,
                }
            }
            Type::Optional(inner) => (*inner).clone(),
            other => other,
        }
    }
}
