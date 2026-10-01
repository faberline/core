use std::collections::HashSet;

use crate::domain::search::index::SearchIndex;
use crate::type_inference::{SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 5. SimilarCode --

pub fn search_similar_code(index: &SearchIndex, q: &SearchQuery, pattern: &str) -> SearchResult {
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut fs = HashSet::new();
    let pat_lower = pattern.to_lowercase();
    let candidates = index.query_by_name(pattern);
    for e in &candidates {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        let hint = e.type_signature.as_deref().unwrap_or("unknown");
        let score = name_similarity(pattern, hint);
        if score > 0.3 {
            let mut m = entry_to_match(e, score);
            m.symbol = Some(hint.to_string());
            matches.push(m);
        }
    }
    if candidates
        .first()
        .and_then(|e| e.type_signature.as_ref())
        .is_some()
    {
        for e in index.query_by_type(pattern) {
            if !entry_in_scope(e, &q.scope) {
                continue;
            }
            fs.insert(e.file.clone());
            let sig = e.type_signature.as_deref().unwrap_or("");
            if sig.to_lowercase().contains(&pat_lower) {
                let mut m = entry_to_match(e, 0.6);
                m.symbol = Some(sig.to_string());
                matches.push(m);
            }
        }
    }
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches.truncate(q.max_results);
    let total = matches.len();
    SearchResult {
        matches,
        total_count: total,
        stats: make_stats(&fs, start),
    }
}

pub(super) fn name_similarity(a: &str, b: &str) -> f64 {
    let (al, bl) = (a.to_lowercase(), b.to_lowercase());
    if al == bl {
        return 1.0;
    }
    if al.is_empty() || bl.is_empty() {
        return 0.0;
    }
    let ac: HashSet<char> = al.chars().collect();
    let bc: HashSet<char> = bl.chars().collect();
    let i = ac.intersection(&bc).count();
    let u = ac.union(&bc).count();
    if u == 0 {
        0.0
    } else {
        i as f64 / u as f64
    }
}
