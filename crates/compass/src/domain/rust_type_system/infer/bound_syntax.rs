use tree_sitter::Node;

use crate::domain::rust_type_system::infer::RustTypeInferencer;
use crate::domain::rust_type_system::types::{Lifetime, RustType, TraitBound, TraitId, TraitRef};

impl RustTypeInferencer {
    /// Parse a type annotation that may include complex trait bounds.
    ///
    /// Handles `Fn(A) -> B + Send + 'static` style compound bounds:
    /// - `Fn(A, B) -> C`: parsed as a `Closure` type
    /// - `+ Send`: additional auto-trait bound
    /// - `+ 'static`: lifetime bound
    ///
    /// Returns a list of trait bounds parsed from the text.
    pub fn parse_trait_bounds_from_node(&mut self, node: &Node, source: &str) -> Vec<TraitBound> {
        let mut bounds = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                // Each bound separated by '+'
                "trait_bound"
                | "type_identifier"
                | "scoped_type_identifier"
                | "generic_type"
                | "function_type" => {
                    let text = &source[child.start_byte()..child.end_byte()];

                    // Check for Fn/FnMut/FnOnce trait syntax
                    if text.starts_with("Fn(")
                        || text.starts_with("FnMut(")
                        || text.starts_with("FnOnce(")
                    {
                        // Parse as function trait bound — we still create a
                        // TraitBound wrapping the Fn trait family.
                        let trait_name = if text.starts_with("FnOnce") {
                            "FnOnce"
                        } else if text.starts_with("FnMut") {
                            "FnMut"
                        } else {
                            "Fn"
                        };

                        bounds.push(TraitBound {
                            trait_ref: TraitRef {
                                trait_id: TraitId(0),
                                name: trait_name.to_string(),
                                type_args: vec![self.parse_type(&child, source)],
                                lifetime_args: vec![],
                            },
                            is_negative: false,
                            higher_ranked_lifetimes: vec![],
                        });
                    } else {
                        // Regular trait bound
                        let name = text.to_string();
                        bounds.push(TraitBound {
                            trait_ref: TraitRef {
                                trait_id: TraitId(0),
                                name,
                                type_args: vec![],
                                lifetime_args: vec![],
                            },
                            is_negative: false,
                            higher_ranked_lifetimes: vec![],
                        });
                    }
                }
                "lifetime" => {
                    // Lifetime bound like 'static
                    let lt_text = &source[child.start_byte()..child.end_byte()];
                    let lt_name = lt_text.trim_start_matches('\'');
                    let lifetime = if lt_name == "static" {
                        Lifetime::Static
                    } else {
                        self.context
                            .lookup_lifetime(lt_name)
                            .unwrap_or(Lifetime::Anonymous)
                    };

                    // Lifetime bounds are represented as a trait bound with
                    // lifetime args on a synthetic outlives predicate.
                    bounds.push(TraitBound {
                        trait_ref: TraitRef {
                            trait_id: TraitId(0),
                            name: format!("'{}", lt_name),
                            type_args: vec![],
                            lifetime_args: vec![lifetime],
                        },
                        is_negative: false,
                        higher_ranked_lifetimes: vec![],
                    });
                }
                // Skip '+' separators and other punctuation
                _ => {}
            }
        }

        bounds
    }

    /// Parse an associated type projection: `<T as Trait>::Item`
    ///
    /// This is a qualified path where we extract:
    /// - The base type `T`
    /// - The trait `Trait`
    /// - The associated type name `Item`
    pub fn parse_qualified_path(&mut self, node: &Node, source: &str) -> RustType {
        // tree-sitter-rust represents `<T as Trait>::Item` as:
        //   scoped_type_identifier or qualified_type
        //     type: qualified_type
        //       type: T
        //       trait: Trait
        //     name: Item
        let text = &source[node.start_byte()..node.end_byte()];

        // Try to parse from the AST structure first
        if let Some(type_node) = node.child_by_field_name("type") {
            let base = self.parse_type(&type_node, source);

            // Look for the "as" trait
            let trait_ref = node.child_by_field_name("trait").map(|trait_node| {
                let trait_name = source[trait_node.start_byte()..trait_node.end_byte()].to_string();
                TraitRef {
                    trait_id: TraitId(0),
                    name: trait_name,
                    type_args: vec![],
                    lifetime_args: vec![],
                }
            });

            // Look for the associated type name
            let assoc_name = node
                .child_by_field_name("name")
                .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();

            if !assoc_name.is_empty() {
                return RustType::Projection {
                    base: Box::new(base),
                    trait_ref,
                    name: assoc_name,
                };
            }
        }

        // Fallback: attempt text-based parsing for `<T as Trait>::Name`
        if text.starts_with('<') {
            if let Some(as_pos) = text.find(" as ") {
                let base_text = &text[1..as_pos];
                let rest = &text[as_pos + 4..];
                if let Some(gt_pos) = rest.find(">::") {
                    let trait_name = &rest[..gt_pos];
                    let assoc_name = &rest[gt_pos + 3..];

                    let base = self.parse_type_from_text(base_text);
                    let trait_ref = Some(TraitRef {
                        trait_id: TraitId(0),
                        name: trait_name.to_string(),
                        type_args: vec![],
                        lifetime_args: vec![],
                    });

                    return RustType::Projection {
                        base: Box::new(base),
                        trait_ref,
                        name: assoc_name.to_string(),
                    };
                }
            }
        }

        RustType::Infer
    }

    /// Parse a type from raw text (fallback for text-based parsing)
    fn parse_type_from_text(&mut self, text: &str) -> RustType {
        let text = text.trim();
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
            "()" => RustType::Unit,
            "!" => RustType::Never,
            "_" => RustType::Infer,
            _ => RustType::Named {
                name: text.to_string(),
                module: None,
                type_args: vec![],
                lifetime_args: vec![],
            },
        }
    }
}
