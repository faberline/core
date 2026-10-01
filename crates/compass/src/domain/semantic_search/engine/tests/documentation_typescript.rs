use super::*;

#[test]
fn test_documentation_search_typescript() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let _file = PathBuf::from("test.ts");

    let code = r#"
/**
 * Validates user input data.
 * @param input - The input string to validate
 * @returns true if valid, false otherwise
 */
function validateInput(input: string): boolean {
    return input.length > 0;
}

/**
 * Processes user data and returns result.
 * @param data - The data to process
 */
function processData(data: any): void {
    console.log(data);
}

function noDocFunction() {
    return 42;
}
"#;

    // First, build symbol table
    let mut symbol_table = SymbolTable::new();
    symbol_table.add_symbol(
        "validateInput".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 6,
                character: 9,
            },
            end: crate::diagnostic::Position {
                line: 6,
                character: 22,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("boolean".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "processData".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 14,
                character: 9,
            },
            end: crate::diagnostic::Position {
                line: 14,
                character: 20,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("void".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "noDocFunction".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 21,
                character: 9,
            },
            end: crate::diagnostic::Position {
                line: 21,
                character: 22,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("number".to_string())),
        }),
        None,
        0,
    );

    // Index the symbol table
    engine.index_symbol_table(PathBuf::from("test.ts"), &symbol_table);

    // Extract docstrings
    let docstrings = engine
        .extract_docstrings(code, Language::TypeScript)
        .unwrap();
    assert_eq!(docstrings.len(), 2);
    assert!(docstrings.contains_key("validateInput"));
    assert!(docstrings.contains_key("processData"));
    assert!(!docstrings.contains_key("noDocFunction"));

    // Update search engine with docstrings
    engine.update_docstrings(docstrings);

    // Search for "validate" in documentation
    let query = SearchQuery {
        kind: SearchKind::ByDocumentation {
            query: "validate".to_string(),
        },
        scope: SearchScope::Project,
        max_results: 10,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());

    let symbols: Vec<String> = result
        .matches
        .iter()
        .filter_map(|m| m.symbol.clone())
        .collect();
    assert!(symbols.contains(&"validateInput".to_string()));
    assert!(!symbols.contains(&"processData".to_string()));
    assert!(!symbols.contains(&"noDocFunction".to_string()));
}
