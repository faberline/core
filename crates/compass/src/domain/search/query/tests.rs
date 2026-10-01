use super::similar_code::name_similarity;
use super::*;
use crate::diagnostic::{Position, Range};
use crate::domain::search::index::SearchIndex;
use crate::semantic::{Symbol, SymbolId, SymbolKind};
use crate::type_inference::SearchKind;
use crate::type_inference::SearchQuery;
use std::path::Path;

fn make_idx(name: &str, doc: Option<&str>) -> SearchIndex {
    let mut idx = SearchIndex::new();
    let sym = Symbol {
        id: SymbolId(0),
        name: name.to_string(),
        kind: SymbolKind::Function,
        location: Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 10,
            },
        },
        type_info: None,
        doc: doc.map(|d| d.to_string()),
        scope_id: 0,
    };
    idx.insert(Path::new("/test.py"), &sym);
    idx
}

fn pq(max: usize) -> SearchQuery {
    SearchQuery {
        kind: SearchKind::ByDocumentation {
            query: String::new(),
        },
        scope: SearchScope::Project,
        max_results: max,
    }
}

#[test]
fn test_doc_search() {
    let r = search_documentation(
        &make_idx("calc", Some("Calculate total price")),
        &pq(10),
        "price",
    );
    assert!(!r.matches.is_empty());
}

#[test]
fn test_usages() {
    let r = search_usages(
        &make_idx("my_func", None),
        &pq(10),
        "my_func",
        Path::new("/test.py"),
    );
    assert_eq!(r.matches.len(), 1);
}

#[test]
fn test_similarity() {
    assert!(name_similarity("foo", "foo") > 0.9);
    assert!(name_similarity("abc", "xyz") < 0.2);
}
