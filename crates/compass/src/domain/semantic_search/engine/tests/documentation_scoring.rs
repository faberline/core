use super::*;

#[test]
fn test_documentation_search_scoring() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let _file = PathBuf::from("test.py");

    let code = r#"
def exact_match():
    """search this exact phrase"""
    pass

def contains_in_middle():
    """Some text before search and more after"""
    pass

def contains_at_end():
    """This has the term at the end: search"""
    pass
"#;

    // First, build symbol table
    let mut symbol_table = SymbolTable::new();
    symbol_table.add_symbol(
        "exact_match".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 1,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 1,
                character: 15,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("None".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "contains_in_middle".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 5,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 5,
                character: 22,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("None".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "contains_at_end".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 9,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 9,
                character: 19,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("None".to_string())),
        }),
        None,
        0,
    );

    // Index the symbol table
    engine.index_symbol_table(PathBuf::from("test.py"), &symbol_table);

    // Extract and update docstrings
    let docstrings = engine.extract_docstrings(code, Language::Python).unwrap();
    engine.update_docstrings(docstrings);

    // Search for "search"
    let query = SearchQuery {
        kind: SearchKind::ByDocumentation {
            query: "search".to_string(),
        },
        scope: SearchScope::Project,
        max_results: 10,
    };

    let result = engine.search(&query);
    assert_eq!(result.len(), 3);

    // Verify that all results have scores
    for match_result in &result.matches {
        assert!(match_result.score > 0.0);
        assert!(match_result.score <= 1.0);
    }

    // The exact match at the start should have the highest score
    let exact_match = result
        .matches
        .iter()
        .find(|m| m.symbol.as_ref().map(|s| s.as_str()) == Some("exact_match"))
        .unwrap();

    let contains_in_middle = result
        .matches
        .iter()
        .find(|m| m.symbol.as_ref().map(|s| s.as_str()) == Some("contains_in_middle"))
        .unwrap();

    assert!(exact_match.score > contains_in_middle.score);
}
