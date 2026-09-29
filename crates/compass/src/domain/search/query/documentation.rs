use std::collections::HashSet;

use crate::domain::search::index::SearchIndex;
use crate::type_inference::{MatchContext, MatchKind, SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 6. DocumentationSearch --

pub fn search_documentation(index: &SearchIndex, q: &SearchQuery, kw: &str) -> SearchResult {
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut fs = HashSet::new();
    let kw_lower = kw.to_lowercase();
    for e in index.query_docs(kw) {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        let doc = e.documentation.as_deref().unwrap_or("");
        let score = doc_score(&kw_lower, doc);
        let lines: Vec<String> = doc.lines().map(|l| l.to_string()).collect();
        let mut m = entry_to_match(e, score);
        m.kind = MatchKind::Documentation;
        m.context = Some(MatchContext {
            before: vec![],
            matched: lines,
            after: vec![],
        });
        matches.push(m);
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

fn doc_score(kw: &str, doc: &str) -> f64 {
    let dl = doc.to_lowercase();
    let mut s = 0.5_f64;
    if dl.starts_with(kw) {
        s += 0.3;
    } else if dl.find(kw).unwrap_or(usize::MAX) < 80 {
        s += 0.15;
    }
    if dl
        .split_whitespace()
        .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == kw)
    {
        s += 0.1;
    }
    s.min(0.95)
}
