use std::collections::HashSet;
use std::path::Path;

use crate::domain::search::index::SearchIndex;
use crate::type_inference::{SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 4. Usages --

pub fn search_usages(
    index: &SearchIndex,
    q: &SearchQuery,
    sym: &str,
    _file: &Path,
) -> SearchResult {
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut fs = HashSet::new();
    for e in index.query_by_name(sym) {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        let mut m = entry_to_match(e, 1.0);
        m.symbol = Some(sym.to_string());
        matches.push(m);
    }
    matches.truncate(q.max_results);
    let total = matches.len();
    SearchResult {
        matches,
        total_count: total,
        stats: make_stats(&fs, start),
    }
}
