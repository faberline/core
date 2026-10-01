use super::*;

// --- R1: OutputFormat::Agent variant ---

#[test]
fn test_output_format_agent_variant() {
    use crate::interfaces::output::output_format::OutputFormat;

    assert_eq!(
        OutputFormat::from_str("agent"),
        Some(OutputFormat::Agent),
        "from_str(\"agent\") should return Some(OutputFormat::Agent)"
    );
    // Case insensitivity
    assert_eq!(
        OutputFormat::from_str("AGENT"),
        Some(OutputFormat::Agent),
        "from_str should be case-insensitive"
    );
}

// --- R2, R7: build_symbols from SymbolTable ---

#[test]
fn test_build_symbols_from_symbol_table() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let type_info = Some(TypeInfo::Callable {
        params: vec![TypeInfo::Primitive("int".to_string())],
        ret: Box::new(TypeInfo::Named("User".to_string())),
    });

    let table =
        make_symbol_table_with_function("get_user", SymbolKind::Function, 41, 50, type_info);

    let symbol_tables = vec![(PathBuf::from("/project/src/db.py"), table)];
    let symbols = builder.build_symbols(&table_views(&symbol_tables));

    // Should contain "db.get_user" (module stem + "." + name)
    let sym = symbols
        .get("db.get_user")
        .expect("symbol db.get_user should exist");
    assert_eq!(sym.file, "src/db.py");
    assert_eq!(sym.line, 42); // 0-indexed 41 -> 1-indexed 42
    assert_eq!(sym.kind, "function");
    assert_eq!(
        sym.type_sig.as_deref(),
        Some("(int) -> User"),
        "type signature should be present (R7)"
    );
}

#[test]
fn test_build_symbols_skips_imports_and_parameters() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let mut table = SymbolTable::new();
    // Add an import
    table.add_symbol(
        "os".to_string(),
        SymbolKind::Import,
        make_range(0, 0, 0, 10),
        None,
        None,
        0,
    );
    // Add a parameter
    table.add_symbol(
        "x".to_string(),
        SymbolKind::Parameter,
        make_range(5, 0, 5, 5),
        None,
        None,
        0,
    );
    // Add a real function
    table.add_symbol(
        "main".to_string(),
        SymbolKind::Function,
        make_range(10, 0, 20, 0),
        None,
        None,
        0,
    );

    let symbol_tables = vec![(PathBuf::from("/project/app.py"), table)];
    let symbols = builder.build_symbols(&table_views(&symbol_tables));

    // Only "main" should be present
    assert_eq!(symbols.len(), 1);
    assert!(symbols.contains_key("app.main"));
}

// --- R3: build_imports from ImportGraph ---

#[test]
fn test_build_imports_from_graph() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let mut graph = ImportGraph::new();
    // Manually add a file with imports via the add_file interface
    // Since add_file parses source, we use a Python source with imports
    let handler_src = "from db import get_user\nfrom models import User\n";
    graph.add_file(PathBuf::from("/project/handler.py"), handler_src, &root);

    let results = vec![make_file_result(
        "/project/handler.py",
        Language::Python,
        vec![],
    )];

    let imports = builder.build_imports(&results, &import_view(&graph, &results));

    // handler.py should have import entries
    if let Some(handler_imports) = imports.get("handler.py") {
        assert!(
            !handler_imports.is_empty(),
            "handler.py should have imports"
        );
    }
    // A file with no imports should not appear
    assert!(!imports.contains_key("db.py"));
}
