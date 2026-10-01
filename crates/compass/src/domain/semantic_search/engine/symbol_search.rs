use std::path::PathBuf;

use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::semantic_search::query::SearchQuery;
use crate::domain::semantic_search::result::{MatchContext, MatchKind, SearchMatch, SearchResult};
use crate::type_inference::Type;

impl SemanticSearchEngine {
    /// Search for implementations of a protocol.
    pub(super) fn search_implementations(
        &self,
        protocol: &str,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();

        // Get all class symbols
        for (_name, locations) in &self.symbol_index {
            for location in locations {
                if matches!(location.kind, MatchKind::ClassDef) {
                    // Check if this class implements the protocol
                    if let Some(ref ty) = location.ty {
                        if self.implements_protocol(&location.name, ty, protocol) {
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
            }

            if result.matches.len() >= query.max_results {
                break;
            }
        }

        result.total_count = result.matches.len();
        result
    }

    /// Check if a class implements a protocol.
    fn implements_protocol(&self, class_name: &str, _class_type: &Type, protocol: &str) -> bool {
        // Use the type context to check protocol satisfaction
        let class_ty = Type::Instance {
            name: class_name.to_string(),
            module: None,
            type_args: vec![],
        };

        // Check if the class satisfies the protocol
        self.inferencer
            .context()
            .satisfies_protocol(&class_ty, protocol)
    }

    /// Search for usages of a symbol.
    pub(super) fn search_usages(
        &self,
        symbol: &str,
        _file: &PathBuf,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();

        if let Some(locations) = self.symbol_index.get(symbol) {
            for loc in locations.iter().take(query.max_results) {
                result.matches.push(SearchMatch {
                    file: loc.file.clone(),
                    span: loc.span,
                    symbol: Some(symbol.to_string()),
                    kind: loc.kind.clone(),
                    score: 1.0,
                    context: None,
                });
            }
            result.total_count = locations.len();
        }

        result
    }

    /// Search for similar code patterns.
    pub(super) fn search_similar_patterns(
        &self,
        pattern: &str,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();
        let pattern_lower = pattern.to_lowercase();

        // Simple pattern matching based on symbol names
        for (_name, locations) in &self.symbol_index {
            for location in locations {
                // Simple heuristic: match if symbol name contains pattern
                if location.name.to_lowercase().contains(&pattern_lower) {
                    result.matches.push(SearchMatch {
                        file: location.file.clone(),
                        span: location.span,
                        symbol: Some(location.name.clone()),
                        kind: location.kind.clone(),
                        score: 0.7, // Lower score for pattern match
                        context: None,
                    });

                    if result.matches.len() >= query.max_results {
                        break;
                    }
                }
            }

            if result.matches.len() >= query.max_results {
                break;
            }
        }

        // Sort by score
        result.matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        result.total_count = result.matches.len();
        result
    }

    /// Search by documentation content.
    pub(super) fn search_by_documentation(
        &self,
        doc_query: &str,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();
        let query_lower = doc_query.to_lowercase();

        // Search through symbols that have documentation
        for (_name, locations) in &self.symbol_index {
            for location in locations {
                // Skip symbols without docstrings
                let docstring = match &location.docstring {
                    Some(doc) => doc,
                    None => continue,
                };

                // Search in the docstring content
                if docstring.to_lowercase().contains(&query_lower) {
                    // Calculate relevance score based on match position and frequency
                    let score: f64 = self.calculate_doc_search_score(&query_lower, docstring);

                    // Convert docstring to context
                    let doc_lines: Vec<String> = docstring.lines().map(|s| s.to_string()).collect();

                    let context = MatchContext {
                        before: Vec::new(),
                        matched: doc_lines,
                        after: Vec::new(),
                    };

                    result.matches.push(SearchMatch {
                        file: location.file.clone(),
                        span: location.span,
                        symbol: Some(location.name.clone()),
                        kind: location.kind.clone(),
                        score,
                        context: Some(context),
                    });

                    if result.matches.len() >= query.max_results {
                        break;
                    }
                }
            }

            if result.matches.len() >= query.max_results {
                break;
            }
        }

        result.total_count = result.matches.len();
        result
    }

    /// Calculate relevance score for documentation search.
    fn calculate_doc_search_score(&self, query: &str, docstring: &str) -> f64 {
        let doc_lower = docstring.to_lowercase();

        // Base score for having a match
        let mut score: f64 = 0.6_f64;

        // Bonus if query appears at the start (likely in summary line)
        if doc_lower.starts_with(query) {
            score += 0.2_f64;
        } else if doc_lower.find(query).unwrap_or(usize::MAX) < 100 {
            // Bonus if match is in first 100 chars
            score += 0.1_f64;
        }

        // Bonus for exact phrase match (not just contains)
        if doc_lower.split_whitespace().any(|word| word == query) {
            score += 0.1_f64;
        }

        // Cap at 0.95 (reserve 1.0 for perfect matches)
        score.min(0.95_f64)
    }
}
