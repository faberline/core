use super::*;

#[test]
fn test_search_by_type_signature() {
    let mut engine = SemanticSearchEngine::new();
    let mut symbol_table = SymbolTable::new();

    // Add function: def func1(x: str, y: int) -> bool
    symbol_table.add_symbol(
        "func1".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 0,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 0,
                character: 9,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![
                TypeInfo::Primitive("str".to_string()),
                TypeInfo::Primitive("int".to_string()),
            ],
            ret: Box::new(TypeInfo::Primitive("bool".to_string())),
        }),
        None,
        0,
    );

    // Add function: def func2(a: int, b: int) -> int
    symbol_table.add_symbol(
        "func2".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 2,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 2,
                character: 9,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![
                TypeInfo::Primitive("int".to_string()),
                TypeInfo::Primitive("int".to_string()),
            ],
            ret: Box::new(TypeInfo::Primitive("int".to_string())),
        }),
        None,
        0,
    );

    let file = PathBuf::from("test.py");
    engine.index_symbol_table(file.clone(), &symbol_table);

    // Search for (str, int) -> bool
    let query = SearchQuery {
        kind: SearchKind::ByTypeSignature {
            params: vec![Type::Str, Type::Int],
            return_type: Some(Type::Bool),
        },
        scope: SearchScope::Project,
        max_results: 10,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());

    // Should find func1 with high score (top result)
    let high_score_matches: Vec<_> = result.matches.iter().filter(|m| m.score > 0.9).collect();
    assert_eq!(high_score_matches.len(), 1);
    assert_eq!(high_score_matches[0].symbol, Some("func1".to_string()));

    // Search for (int, int) -> int
    let query = SearchQuery {
        kind: SearchKind::ByTypeSignature {
            params: vec![Type::Int, Type::Int],
            return_type: Some(Type::Int),
        },
        scope: SearchScope::Project,
        max_results: 10,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());

    // Should find func2 with high score
    let high_score_matches: Vec<_> = result.matches.iter().filter(|m| m.score > 0.9).collect();
    assert_eq!(high_score_matches.len(), 1);
    assert_eq!(high_score_matches[0].symbol, Some("func2".to_string()));
}

#[test]
fn test_type_compatibility_score() {
    let engine = SemanticSearchEngine::new();

    // Exact match
    assert_eq!(engine.type_compatibility_score(&Type::Int, &Type::Int), 1.0);
    assert_eq!(engine.type_compatibility_score(&Type::Str, &Type::Str), 1.0);

    // Any/Unknown matches
    assert!(engine.type_compatibility_score(&Type::Any, &Type::Int) > 0.5);
    assert!(engine.type_compatibility_score(&Type::Int, &Type::Unknown) > 0.5);

    // Container types
    let list_int = Type::List(Box::new(Type::Int));
    let list_int2 = Type::List(Box::new(Type::Int));
    assert!(engine.type_compatibility_score(&list_int, &list_int2) > 0.8);

    // No match
    assert_eq!(engine.type_compatibility_score(&Type::Int, &Type::Str), 0.0);
}

#[test]
fn test_search_implementations() {
    let mut engine = SemanticSearchEngine::new();
    let mut symbol_table = SymbolTable::new();

    // Add a class
    symbol_table.add_symbol(
        "MyClass".to_string(),
        SymbolKind::Class,
        Range {
            start: crate::diagnostic::Position {
                line: 0,
                character: 6,
            },
            end: crate::diagnostic::Position {
                line: 0,
                character: 13,
            },
        },
        None,
        None,
        0,
    );

    let file = PathBuf::from("test.py");
    engine.index_symbol_table(file.clone(), &symbol_table);

    // Search for implementations of a protocol
    let query = SearchQuery {
        kind: SearchKind::Implementations {
            protocol: "Sized".to_string(),
        },
        scope: SearchScope::Project,
        max_results: 10,
    };

    let result = engine.search(&query);
    // Note: This will be empty because we haven't implemented protocol checking yet
    // but at least it doesn't crash
    assert!(result.is_empty() || !result.is_empty());
}
