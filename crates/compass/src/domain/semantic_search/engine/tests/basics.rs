use super::*;

#[test]
fn test_search_result() {
    let result = SearchResult::empty();
    assert!(result.is_empty());
    assert_eq!(result.len(), 0);
}

#[test]
fn test_search_engine() {
    let engine = SemanticSearchEngine::new();
    let query = SearchQuery {
        kind: SearchKind::Usages {
            symbol: "foo".to_string(),
            file: PathBuf::from("test.py"),
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert!(result.is_empty());
}

#[test]
fn test_index_population() {
    let mut engine = SemanticSearchEngine::new();
    let mut symbol_table = SymbolTable::new();

    // Add test symbols
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
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("None".to_string())),
        }),
        None,
        0,
    );

    symbol_table.add_symbol(
        "MyClass".to_string(),
        SymbolKind::Class,
        Range {
            start: crate::diagnostic::Position {
                line: 2,
                character: 6,
            },
            end: crate::diagnostic::Position {
                line: 2,
                character: 13,
            },
        },
        None,
        None,
        0,
    );

    let file = PathBuf::from("test.py");
    engine.index_symbol_table(file.clone(), &symbol_table);

    // Search for func1
    let query = SearchQuery {
        kind: SearchKind::Usages {
            symbol: "func1".to_string(),
            file: file.clone(),
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());
    assert_eq!(result.len(), 1);
    assert_eq!(result.matches[0].symbol, Some("func1".to_string()));

    // Search for MyClass
    let query = SearchQuery {
        kind: SearchKind::Usages {
            symbol: "MyClass".to_string(),
            file,
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());
    assert_eq!(result.len(), 1);
    assert_eq!(result.matches[0].symbol, Some("MyClass".to_string()));
}

#[test]
fn test_convert_type_info() {
    // Test primitive types
    let int_type = SemanticSearchEngine::convert_type_info(&TypeInfo::Primitive("int".to_string()));
    assert_eq!(int_type, Type::Int);

    // Test list types
    let list_type = SemanticSearchEngine::convert_type_info(&TypeInfo::List(Box::new(
        TypeInfo::Primitive("str".to_string()),
    )));
    assert_eq!(list_type, Type::List(Box::new(Type::Str)));

    // Test optional types
    let opt_type = SemanticSearchEngine::convert_type_info(&TypeInfo::Optional(Box::new(
        TypeInfo::Primitive("int".to_string()),
    )));
    assert_eq!(opt_type, Type::Union(vec![Type::Int, Type::None]));
}
