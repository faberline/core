use std::collections::{HashSet, VecDeque};
use std::path::Path;

use crate::domain::search::index::SearchIndex;
use crate::type_inference::{CallDirection, MatchKind, SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 2. CallHierarchy --

/// Lightweight call graph for BFS traversal.
#[derive(Debug, Default)]
pub struct CallGraphIndex {
    pub calls: std::collections::HashMap<String, Vec<String>>,
    pub called_by: std::collections::HashMap<String, Vec<String>>,
}

pub fn search_call_hierarchy(
    index: &SearchIndex,
    cg: &CallGraphIndex,
    q: &SearchQuery,
    symbol: &str,
    _file: &Path,
    dir: CallDirection,
    max_depth: usize,
) -> SearchResult {
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut visited = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((symbol.to_string(), 0));
    while let Some((cur, depth)) = queue.pop_front() {
        if !visited.insert(cur.clone()) || depth > max_depth {
            continue;
        }
        if matches.len() >= q.max_results {
            break;
        }
        let nbrs = match dir {
            CallDirection::Callers => cg.called_by.get(&cur),
            CallDirection::Callees => cg.calls.get(&cur),
        };
        if let Some(names) = nbrs {
            for name in names {
                for e in index.query_by_name(name) {
                    if !entry_in_scope(e, &q.scope) {
                        continue;
                    }
                    let mut m = entry_to_match(e, 1.0 - depth as f64 * 0.1);
                    m.symbol = Some(name.clone());
                    m.kind = MatchKind::Call;
                    matches.push(m);
                }
                queue.push_back((name.clone(), depth + 1));
            }
        }
    }
    matches.truncate(q.max_results);
    let total = matches.len();
    SearchResult {
        matches,
        total_count: total,
        stats: make_stats(&HashSet::new(), start),
    }
}
