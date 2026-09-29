use std::path::PathBuf;

use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::semantic_search::query::{CallDirection, SearchQuery, TypeHierarchyDirection};
use crate::domain::semantic_search::result::{MatchKind, SearchMatch, SearchResult};
use crate::type_inference::Type;

impl SemanticSearchEngine {
    /// Search call hierarchy.
    pub(super) fn search_call_hierarchy(
        &self,
        symbol: &str,
        _file: &PathBuf,
        direction: CallDirection,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();
        let mut visited = std::collections::HashSet::new();

        // Recursively collect call hierarchy
        self.collect_call_hierarchy(
            symbol,
            direction,
            &mut visited,
            &mut result,
            query.max_results,
        );

        result.total_count = result.matches.len();
        result
    }

    /// Recursively collect call hierarchy.
    fn collect_call_hierarchy(
        &self,
        symbol: &str,
        direction: CallDirection,
        visited: &mut std::collections::HashSet<String>,
        result: &mut SearchResult,
        max_results: usize,
    ) {
        if visited.contains(symbol) || result.matches.len() >= max_results {
            return;
        }

        visited.insert(symbol.to_string());

        let call_sites = match direction {
            CallDirection::Callers => self.call_graph.get_callers(symbol),
            CallDirection::Callees => self.call_graph.get_callees(symbol),
        };

        for site in call_sites {
            let related_symbol = match direction {
                CallDirection::Callers => &site.caller,
                CallDirection::Callees => &site.callee,
            };

            result.matches.push(SearchMatch {
                file: site.file.clone(),
                span: site.span,
                symbol: Some(related_symbol.clone()),
                kind: MatchKind::Call,
                score: 1.0,
                context: None,
            });

            if result.matches.len() >= max_results {
                return;
            }

            // Recursively search (limit depth to avoid infinite loops)
            if visited.len() < 100 {
                self.collect_call_hierarchy(
                    related_symbol,
                    direction,
                    visited,
                    result,
                    max_results,
                );
            }
        }
    }

    /// Search type hierarchy.
    pub(super) fn search_type_hierarchy(
        &self,
        type_name: &str,
        direction: TypeHierarchyDirection,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();
        let type_ctx = self.inferencer.context();

        match direction {
            TypeHierarchyDirection::Supertypes => {
                // Find parent types by checking all class/interface symbols
                for (_name, locations) in &self.symbol_index {
                    for location in locations {
                        if matches!(location.kind, MatchKind::ClassDef) {
                            let inst_ty = Type::Instance {
                                name: type_name.to_string(),
                                module: None,
                                type_args: vec![],
                            };

                            // Check if type_name satisfies this protocol/interface
                            if type_ctx.satisfies_protocol(&inst_ty, &location.name) {
                                result.matches.push(SearchMatch {
                                    file: location.file.clone(),
                                    span: location.span,
                                    symbol: Some(location.name.clone()),
                                    kind: location.kind.clone(),
                                    score: 1.0,
                                    context: None,
                                });

                                if result.matches.len() >= query.max_results {
                                    break;
                                }
                            }
                        }
                    }

                    if result.matches.len() >= query.max_results {
                        break;
                    }
                }
            }
            TypeHierarchyDirection::Subtypes => {
                // Find child types that implement this protocol
                for (_name, locations) in &self.symbol_index {
                    for location in locations {
                        if matches!(location.kind, MatchKind::ClassDef) {
                            let class_ty = Type::Instance {
                                name: location.name.clone(),
                                module: None,
                                type_args: vec![],
                            };

                            if type_ctx.satisfies_protocol(&class_ty, type_name) {
                                result.matches.push(SearchMatch {
                                    file: location.file.clone(),
                                    span: location.span,
                                    symbol: Some(location.name.clone()),
                                    kind: location.kind.clone(),
                                    score: 1.0,
                                    context: None,
                                });

                                if result.matches.len() >= query.max_results {
                                    break;
                                }
                            }
                        }
                    }

                    if result.matches.len() >= query.max_results {
                        break;
                    }
                }
            }
            TypeHierarchyDirection::Both => {
                // Search both directions
                let supertypes = self.search_type_hierarchy(
                    type_name,
                    TypeHierarchyDirection::Supertypes,
                    query,
                );
                let subtypes =
                    self.search_type_hierarchy(type_name, TypeHierarchyDirection::Subtypes, query);

                result.matches.extend(supertypes.matches);
                result.matches.extend(subtypes.matches);
                result.matches.truncate(query.max_results);
            }
        }

        result.total_count = result.matches.len();
        result
    }
}
