use std::collections::HashSet;

use crate::domain::search::index::{IndexSymbolKind, SearchIndex};
use crate::type_inference::{SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 3. Implementations --

pub fn search_implementations(index: &SearchIndex, q: &SearchQuery, proto: &str) -> SearchResult {
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut fs = HashSet::new();
    for e in &index.query_by_name(proto) {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        if matches!(
            e.kind,
            IndexSymbolKind::Impl | IndexSymbolKind::Class | IndexSymbolKind::Struct
        ) {
            let mut m = entry_to_match(e, 1.0);
            m.symbol = Some(proto.to_string());
            matches.push(m);
        }
    }
    for e in index.query_by_type(proto) {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        if matches!(
            e.kind,
            IndexSymbolKind::Impl
                | IndexSymbolKind::Class
                | IndexSymbolKind::Struct
                | IndexSymbolKind::Trait
                | IndexSymbolKind::Interface
        ) {
            let mut m = entry_to_match(e, 0.8);
            m.symbol = Some(proto.to_string());
            matches.push(m);
        }
    }
    matches.truncate(q.max_results);
    let total = matches.len();
    SearchResult {
        matches,
        total_count: total,
        stats: make_stats(&fs, start),
    }
}
