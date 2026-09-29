use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::semantic_search::query::SearchQuery;
use crate::domain::semantic_search::result::{SearchMatch, SearchResult};
use crate::type_inference::Type;

impl SemanticSearchEngine {
    /// Search by type signature.
    pub(super) fn search_by_type_signature(
        &self,
        params: &[Type],
        return_type: Option<&Type>,
        query: &SearchQuery,
    ) -> SearchResult {
        let mut result = SearchResult::empty();
        let mut all_matches: Vec<(SearchMatch, f64)> = Vec::new();

        // Search for functions with matching signatures
        for (_sig_key, locations) in &self.type_signature_index {
            for location in locations {
                if let Some(ref loc_type) = location.ty {
                    if let Type::Callable {
                        params: loc_params,
                        ret: loc_ret,
                    } = loc_type
                    {
                        let score = self.compute_signature_match_score(
                            params,
                            return_type,
                            loc_params,
                            loc_ret,
                        );

                        if score > 0.0 {
                            let search_match = SearchMatch {
                                file: location.file.clone(),
                                span: location.span,
                                symbol: Some(location.name.clone()),
                                kind: location.kind.clone(),
                                score,
                                context: None,
                            };
                            all_matches.push((search_match, score));
                        }
                    }
                }
            }
        }

        // Sort by score (descending)
        all_matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top results
        result.matches = all_matches
            .into_iter()
            .take(query.max_results)
            .map(|(m, _)| m)
            .collect();

        result.total_count = result.matches.len();
        result
    }

    /// Compute match score between two function signatures.
    /// Returns 0.0 for no match, 1.0 for exact match.
    fn compute_signature_match_score(
        &self,
        query_params: &[Type],
        query_return: Option<&Type>,
        func_params: &[crate::type_inference::Param],
        func_return: &Type,
    ) -> f64 {
        let mut score = 0.0;
        let mut components = 0;

        // Check parameter count
        if query_params.len() != func_params.len() {
            // Allow variadic functions (partial match)
            if query_params.len() < func_params.len() {
                score += 0.3;
            } else {
                return 0.0; // Too many parameters
            }
        } else {
            score += 1.0;
        }
        components += 1;

        // Check parameter types
        for (i, query_param) in query_params.iter().enumerate() {
            if let Some(func_param) = func_params.get(i) {
                let param_score = self.type_compatibility_score(query_param, &func_param.ty);
                score += param_score;
                components += 1;
            }
        }

        // Check return type
        if let Some(query_ret) = query_return {
            let ret_score = self.type_compatibility_score(query_ret, func_return);
            score += ret_score * 1.5; // Weight return type higher
            components += 1;
        }

        if components > 0 {
            score / components as f64
        } else {
            0.0
        }
    }

    /// Compute compatibility score between two types (0.0 to 1.0).
    pub(super) fn type_compatibility_score(&self, expected: &Type, actual: &Type) -> f64 {
        // Exact match
        if expected == actual {
            return 1.0;
        }

        // Any/Unknown matches everything
        if matches!(expected, Type::Any | Type::Unknown)
            || matches!(actual, Type::Any | Type::Unknown)
        {
            return 0.8;
        }

        match (expected, actual) {
            // Union types
            (Type::Union(types), _) => types
                .iter()
                .map(|t| self.type_compatibility_score(t, actual))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0),
            (_, Type::Union(types)) => types
                .iter()
                .map(|t| self.type_compatibility_score(expected, t))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0),

            // Container types (covariant in element type)
            (Type::List(e1), Type::List(e2)) => 0.9 * self.type_compatibility_score(e1, e2),
            (Type::Set(e1), Type::Set(e2)) => 0.9 * self.type_compatibility_score(e1, e2),
            (Type::Dict(k1, v1), Type::Dict(k2, v2)) => {
                let k_score = self.type_compatibility_score(k1, k2);
                let v_score = self.type_compatibility_score(v1, v2);
                0.9 * (k_score + v_score) / 2.0
            }

            // Optional types
            (Type::Optional(inner), other) | (other, Type::Optional(inner)) => {
                0.8 * self.type_compatibility_score(inner, other)
            }

            // Instance types (check if same class name)
            (Type::Instance { name: n1, .. }, Type::Instance { name: n2, .. }) => {
                if n1 == n2 {
                    0.9
                } else {
                    0.0
                }
            }

            // Class types
            (Type::ClassType { name: n1, .. }, Type::ClassType { name: n2, .. }) => {
                if n1 == n2 {
                    0.9
                } else {
                    0.0
                }
            }

            _ => 0.0,
        }
    }
}
