pub(crate) mod call_hierarchy;
pub(crate) mod documentation;
pub(crate) mod implementations;
pub(crate) mod similar_code;
#[cfg(test)]
mod tests;
pub(crate) mod type_signature;
pub(crate) mod usages;

use std::collections::HashSet;

use crate::domain::search::index::{IndexSymbolKind, SymbolEntry};
use crate::type_inference::{MatchKind, SearchMatch, SearchScope, SearchStats, Span};

pub use call_hierarchy::{search_call_hierarchy, CallGraphIndex};
pub use documentation::search_documentation;
pub use implementations::search_implementations;
pub use similar_code::search_similar_code;
pub use type_signature::search_by_type_signature;
pub use usages::search_usages;

// -- Shared helpers --

fn entry_in_scope(e: &SymbolEntry, scope: &SearchScope) -> bool {
    match scope {
        SearchScope::CurrentFile(f) => e.file == *f,
        SearchScope::Files(fs) => fs.contains(&e.file),
        SearchScope::Project | SearchScope::ProjectWithDeps => true,
    }
}

fn to_match_kind(kind: IndexSymbolKind) -> MatchKind {
    match kind {
        IndexSymbolKind::Function => MatchKind::FunctionDef,
        IndexSymbolKind::Class | IndexSymbolKind::Struct => MatchKind::ClassDef,
        IndexSymbolKind::Variable | IndexSymbolKind::Const | IndexSymbolKind::Static => {
            MatchKind::VariableAssignment
        }
        IndexSymbolKind::Import => MatchKind::Import,
        IndexSymbolKind::Interface | IndexSymbolKind::Trait => MatchKind::ClassDef,
        _ => MatchKind::FunctionDef,
    }
}

fn entry_to_match(e: &SymbolEntry, score: f64) -> SearchMatch {
    SearchMatch {
        file: e.file.clone(),
        span: Span {
            start: 0,
            end: 0,
            start_line: e.position.start_line as usize,
            start_col: e.position.start_col as usize,
            end_line: e.position.end_line as usize,
            end_col: e.position.end_col as usize,
        },
        symbol: None,
        kind: to_match_kind(e.kind),
        score,
        context: None,
    }
}

fn make_stats(files: &HashSet<std::path::PathBuf>, start: std::time::Instant) -> SearchStats {
    SearchStats {
        files_searched: files.len(),
        time_ms: start.elapsed().as_millis() as u64,
        truncated: false,
    }
}
