use tree_sitter::Node;

use crate::domain::rust_type_system::symbols::RustSymbolCollector;
use crate::domain::rust_type_system::types::{
    Lifetime, LifetimeId, RustType, TraitBound, TraitId, TraitRef, WherePredicate,
};

impl RustSymbolCollector {
    pub(super) fn collect_supertraits(&self, node: &Node, source: &str) -> Vec<TraitBound> {
        let mut bounds = Vec::new();

        // Look for trait_bounds node (after the colon in trait definition)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "trait_bounds" {
                bounds.extend(self.parse_trait_bounds(&child, source));
            }
        }

        bounds
    }

    pub(super) fn parse_trait_bounds(&self, node: &Node, source: &str) -> Vec<TraitBound> {
        let mut bounds = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "scoped_type_identifier" | "generic_type" => {
                    let trait_ref = self.parse_trait_ref(&child, source);
                    bounds.push(TraitBound {
                        trait_ref,
                        is_negative: false,
                        higher_ranked_lifetimes: vec![],
                    });
                }
                "removed_trait_bound" => {
                    // Negative bound like !Send
                    if let Some(inner) = child.child(1) {
                        let trait_ref = self.parse_trait_ref(&inner, source);
                        bounds.push(TraitBound {
                            trait_ref,
                            is_negative: true,
                            higher_ranked_lifetimes: vec![],
                        });
                    }
                }
                "higher_ranked_trait_bound" => {
                    // for<'a> Trait<'a>
                    let lifetimes = self.collect_higher_ranked_lifetimes(&child, source);
                    if let Some(trait_node) = child.child_by_field_name("type") {
                        let trait_ref = self.parse_trait_ref(&trait_node, source);
                        bounds.push(TraitBound {
                            trait_ref,
                            is_negative: false,
                            higher_ranked_lifetimes: lifetimes,
                        });
                    }
                }
                _ => {}
            }
        }

        bounds
    }

    fn parse_trait_ref(&self, node: &Node, source: &str) -> TraitRef {
        let name = source[node.start_byte()..node.end_byte()].to_string();
        let mut type_args = Vec::new();
        let mut lifetime_args = Vec::new();

        // Check for generic arguments
        if node.kind() == "generic_type" {
            if let Some(args_node) = node.child_by_field_name("type_arguments") {
                let mut cursor = args_node.walk();
                for child in args_node.children(&mut cursor) {
                    match child.kind() {
                        "lifetime" => {
                            let lt_name = source[child.start_byte()..child.end_byte()].to_string();
                            lifetime_args.push(Lifetime::Named {
                                id: LifetimeId(0),
                                name: lt_name,
                            });
                        }
                        _ if !matches!(child.kind(), "<" | ">" | ",") => {
                            type_args.push(self.parse_type(&child, source));
                        }
                        _ => {}
                    }
                }
            }
        }

        TraitRef {
            trait_id: TraitId(0), // Will be resolved during semantic analysis
            name,
            type_args,
            lifetime_args,
        }
    }

    fn collect_higher_ranked_lifetimes(&self, node: &Node, source: &str) -> Vec<Lifetime> {
        let mut lifetimes = Vec::new();

        if let Some(params) = node.child_by_field_name("type_parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                if child.kind() == "lifetime" {
                    let name = source[child.start_byte()..child.end_byte()].to_string();
                    lifetimes.push(Lifetime::Named {
                        id: LifetimeId(0),
                        name,
                    });
                }
            }
        }

        lifetimes
    }

    pub(super) fn collect_impl_trait(&self, node: &Node, source: &str) -> Option<TraitRef> {
        node.child_by_field_name("trait")
            .map(|n| self.parse_trait_ref(&n, source))
    }

    pub(super) fn collect_impl_type(&self, node: &Node, source: &str) -> RustType {
        node.child_by_field_name("type")
            .map(|n| self.parse_type(&n, source))
            .unwrap_or(RustType::Infer)
    }

    pub(super) fn collect_where_clause(&self, node: &Node, source: &str) -> Vec<WherePredicate> {
        let mut predicates = Vec::new();

        // Find the where_clause child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "where_clause" {
                predicates.extend(self.parse_where_clause(&child, source));
            }
        }

        predicates
    }

    fn parse_where_clause(&self, node: &Node, source: &str) -> Vec<WherePredicate> {
        let mut predicates = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "where_predicate" {
                if let Some(pred) = self.parse_where_predicate(&child, source) {
                    predicates.push(pred);
                }
            }
        }

        predicates
    }

    fn parse_where_predicate(&self, node: &Node, source: &str) -> Option<WherePredicate> {
        // Check for lifetime bound: 'a: 'b
        if let Some(lifetime) = node.child_by_field_name("left") {
            if lifetime.kind() == "lifetime" {
                let lt_name = source[lifetime.start_byte()..lifetime.end_byte()].to_string();
                let lifetime = Lifetime::Named {
                    id: LifetimeId(0),
                    name: lt_name,
                };

                let mut bounds = Vec::new();
                if let Some(bounds_node) = node.child_by_field_name("bounds") {
                    let mut cursor = bounds_node.walk();
                    for child in bounds_node.children(&mut cursor) {
                        if child.kind() == "lifetime" {
                            let name = source[child.start_byte()..child.end_byte()].to_string();
                            bounds.push(Lifetime::Named {
                                id: LifetimeId(0),
                                name,
                            });
                        }
                    }
                }

                return Some(WherePredicate::LifetimeBound { lifetime, bounds });
            }
        }

        // Type bound: T: Trait
        if let Some(type_node) = node.child_by_field_name("left") {
            let ty = self.parse_type(&type_node, source);

            let mut bounds = Vec::new();
            if let Some(bounds_node) = node.child_by_field_name("bounds") {
                bounds = self.parse_trait_bounds(&bounds_node, source);
            }

            return Some(WherePredicate::TypeBound { ty, bounds });
        }

        None
    }
}
