use super::*;

#[test]
fn test_documentation_search_python() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let _file = PathBuf::from("test.py");

    let code = r#"
def calculate_sum(a, b):
    """
    Calculate the sum of two numbers.

    Args:
        a: First number
        b: Second number

    Returns:
        The sum of a and b
    """
    return a + b

def calculate_product(a, b):
    """
    Calculate the product of two numbers.

    Args:
        a: First number
        b: Second number

    Returns:
        The product of a and b
    """
    return a * b

def unrelated_function():
    """Do something unrelated."""
    pass
"#;

    // First, build symbol table
    let mut symbol_table = SymbolTable::new();
    symbol_table.add_symbol(
        "calculate_sum".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 1,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 1,
                character: 17,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("int".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "calculate_product".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 15,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 15,
                character: 21,
            },
        },
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("int".to_string())),
        }),
        None,
        0,
    );
    symbol_table.add_symbol(
        "unrelated_function".to_string(),
        SymbolKind::Function,
        Range {
            start: crate::diagnostic::Position {
                line: 29,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 29,
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

    // Index the symbol table
    engine.index_symbol_table(PathBuf::from("test.py"), &symbol_table);

    // Extract docstrings
    let docstrings = engine.extract_docstrings(code, Language::Python).unwrap();
    assert_eq!(docstrings.len(), 3);
    assert!(docstrings.contains_key("calculate_sum"));
    assert!(docstrings.contains_key("calculate_product"));

    // Update search engine with docstrings
    engine.update_docstrings(docstrings);

    // Search for "sum" in documentation
    let query = SearchQuery {
        kind: SearchKind::ByDocumentation {
            query: "sum".to_string(),
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
    assert!(symbols.contains(&"calculate_sum".to_string()));
    assert!(!symbols.contains(&"unrelated_function".to_string()));

    // Verify context is included
    assert!(result.matches[0].context.is_some());
    if let Some(context) = &result.matches[0].context {
        assert!(!context.matched.is_empty());
    }
}
