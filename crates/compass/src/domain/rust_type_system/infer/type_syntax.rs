use tree_sitter::Node;

use crate::domain::rust_type_system::infer::RustTypeInferencer;
use crate::domain::rust_type_system::types::{Lifetime, RustType};

impl RustTypeInferencer {
    /// Parse a type annotation
    pub(super) fn parse_type(&mut self, node: &Node, source: &str) -> RustType {
        let kind = node.kind();

        match kind {
            "primitive_type" => {
                let text = &source[node.start_byte()..node.end_byte()];
                match text {
                    "bool" => RustType::Bool,
                    "char" => RustType::Char,
                    "str" => RustType::Str,
                    "i8" => RustType::I8,
                    "i16" => RustType::I16,
                    "i32" => RustType::I32,
                    "i64" => RustType::I64,
                    "i128" => RustType::I128,
                    "isize" => RustType::Isize,
                    "u8" => RustType::U8,
                    "u16" => RustType::U16,
                    "u32" => RustType::U32,
                    "u64" => RustType::U64,
                    "u128" => RustType::U128,
                    "usize" => RustType::Usize,
                    "f32" => RustType::F32,
                    "f64" => RustType::F64,
                    _ => RustType::Infer,
                }
            }
            "type_identifier" => {
                let name = &source[node.start_byte()..node.end_byte()];
                RustType::Named {
                    name: name.to_string(),
                    module: None,
                    type_args: vec![],
                    lifetime_args: vec![],
                }
            }
            "reference_type" => {
                let mutable = node.child_by_field_name("mutable_specifier").is_some();
                let inner = if let Some(type_node) = node.child_by_field_name("type") {
                    self.parse_type(&type_node, source)
                } else {
                    RustType::Infer
                };

                RustType::Reference {
                    lifetime: None,
                    mutable,
                    inner: Box::new(inner),
                }
            }
            "tuple_type" => {
                let mut elements = Vec::new();
                let mut cursor = node.walk();

                for child in node.children(&mut cursor) {
                    if !matches!(child.kind(), "(" | ")" | ",") {
                        elements.push(self.parse_type(&child, source));
                    }
                }

                RustType::Tuple(elements)
            }
            "array_type" => {
                let element = if let Some(elem) = node.child_by_field_name("element") {
                    self.parse_type(&elem, source)
                } else {
                    RustType::Infer
                };

                // Parse size expression: [T; N]
                // The size is the "length" field in tree-sitter-rust, or
                // the expression after the semicolon.
                let size = node
                    .child_by_field_name("length")
                    .and_then(|len_node| {
                        let text = &source[len_node.start_byte()..len_node.end_byte()];
                        self.parse_const_size_expr(text)
                    })
                    .unwrap_or(0);

                RustType::Array {
                    element: Box::new(element),
                    size,
                }
            }
            "unit_type" => RustType::Unit,
            "never_type" => RustType::Never,
            // Associated type projection: <T as Trait>::Item
            "scoped_type_identifier" | "qualified_type" => self.parse_qualified_path(node, source),
            // Generic type with type arguments: Vec<i32>
            "generic_type" => {
                let name = node
                    .child_by_field_name("type")
                    .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                    .unwrap_or_default();

                let type_arg_nodes: Vec<Node> = node
                    .child_by_field_name("type_arguments")
                    .map(|args| {
                        let mut cursor = args.walk();
                        args.children(&mut cursor)
                            .filter(|c| !matches!(c.kind(), "<" | ">" | "," | "lifetime"))
                            .collect()
                    })
                    .unwrap_or_default();

                let type_args: Vec<RustType> = type_arg_nodes
                    .iter()
                    .map(|c| self.parse_type(c, source))
                    .collect();

                let lifetime_args: Vec<Lifetime> = node
                    .child_by_field_name("type_arguments")
                    .map(|args| {
                        let mut cursor = args.walk();
                        args.children(&mut cursor)
                            .filter(|c| c.kind() == "lifetime")
                            .map(|c| {
                                let lt_text = &source[c.start_byte()..c.end_byte()];
                                let lt_name = lt_text.trim_start_matches('\'');
                                if lt_name == "static" {
                                    Lifetime::Static
                                } else {
                                    self.context
                                        .lookup_lifetime(lt_name)
                                        .unwrap_or(Lifetime::Anonymous)
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                RustType::Named {
                    name,
                    module: None,
                    type_args,
                    lifetime_args,
                }
            }
            // Trait object: dyn Trait
            "dynamic_type" => {
                let mut bounds = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() != "dyn" {
                        bounds.extend(self.parse_trait_bounds_from_node(&child, source));
                    }
                }
                RustType::TraitObject {
                    bounds,
                    lifetime: None,
                }
            }
            _ => RustType::Infer,
        }
    }

    /// Parse a const size expression for array types `[T; N]`.
    ///
    /// Handles integer literals, simple arithmetic on literals (e.g., `2 + 3`),
    /// and common const expressions. Returns `None` for expressions that
    /// cannot be evaluated statically (e.g., const generic parameters).
    fn parse_const_size_expr(&self, text: &str) -> Option<usize> {
        let text = text.trim();

        // Direct integer literal
        if let Ok(n) = text.parse::<usize>() {
            return Some(n);
        }

        // Handle hex literals: 0x...
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            return usize::from_str_radix(hex, 16).ok();
        }

        // Handle binary literals: 0b...
        if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
            return usize::from_str_radix(bin, 2).ok();
        }

        // Handle octal literals: 0o...
        if let Some(oct) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
            return usize::from_str_radix(oct, 8).ok();
        }

        // Handle underscored literals: 1_000_000
        let no_underscores = text.replace('_', "");
        if let Ok(n) = no_underscores.parse::<usize>() {
            return Some(n);
        }

        // Handle simple binary arithmetic: A + B, A * B, A - B
        for op in [" + ", " * ", " - "] {
            if let Some(pos) = text.find(op) {
                let left = text[..pos].trim();
                let right = text[pos + op.len()..].trim();
                if let (Some(l), Some(r)) = (
                    self.parse_const_size_expr(left),
                    self.parse_const_size_expr(right),
                ) {
                    return match op.trim() {
                        "+" => Some(l + r),
                        "*" => Some(l * r),
                        "-" => l.checked_sub(r),
                        _ => None,
                    };
                }
            }
        }

        // Cannot evaluate — likely a const generic parameter (N)
        None
    }
}
