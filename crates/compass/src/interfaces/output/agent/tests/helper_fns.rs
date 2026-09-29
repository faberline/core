use super::*;

// --- Helper function tests ---

#[test]
fn test_format_qualified_name() {
    assert_eq!(
        format_qualified_name("src/db.py", "get_user"),
        "db.get_user"
    );
    assert_eq!(
        format_qualified_name("handler.py", "handle"),
        "handler.handle"
    );
    assert_eq!(
        format_qualified_name("a/b/models.ts", "User"),
        "models.User"
    );
}

#[test]
fn test_symbol_kind_to_agent_kind() {
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Function), "function");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Class), "class");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Struct), "class");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Trait), "interface");
    assert_eq!(
        symbol_kind_to_agent_kind(SymbolKind::Interface),
        "interface"
    );
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Variable), "variable");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Const), "constant");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Static), "constant");
    assert_eq!(
        symbol_kind_to_agent_kind(SymbolKind::TypeAlias),
        "type_alias"
    );
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Module), "module");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Decorator), "function");
    assert_eq!(symbol_kind_to_agent_kind(SymbolKind::Macro), "function");
}

#[test]
fn test_severity_to_str() {
    assert_eq!(severity_to_str(DiagnosticSeverity::Error), "error");
    assert_eq!(severity_to_str(DiagnosticSeverity::Warning), "warning");
    assert_eq!(severity_to_str(DiagnosticSeverity::Information), "info");
    assert_eq!(severity_to_str(DiagnosticSeverity::Hint), "hint");
}

#[test]
fn test_find_enclosing_symbol_innermost() {
    // Nested symbols: outer at lines 0-30, inner at lines 5-15
    let mut table = SymbolTable::new();
    table.add_symbol(
        "MyClass".to_string(),
        SymbolKind::Class,
        make_range(0, 0, 30, 0),
        None,
        None,
        0,
    );
    table.add_symbol(
        "my_method".to_string(),
        SymbolKind::Function,
        make_range(5, 4, 15, 4),
        None,
        None,
        1,
    );

    // Position at line 10 should find the inner method
    let result = find_enclosing_symbol(&table, 10, 8);
    assert_eq!(
        result,
        Some("my_method".to_string()),
        "should find innermost enclosing symbol"
    );
}

#[test]
fn test_find_enclosing_symbol_none() {
    let table = make_symbol_table_with_function("func", SymbolKind::Function, 10, 20, None);

    // Position before any symbol
    let result = find_enclosing_symbol(&table, 5, 0);
    assert_eq!(
        result, None,
        "should return None when no symbol encloses the position"
    );
}

#[test]
fn test_build_full_output_with_all_fields() {
    // Integration-style test: build complete output with symbols, imports, issues, impact
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let mut table = SymbolTable::new();
    let sym_id = table.add_symbol(
        "handler".to_string(),
        SymbolKind::Function,
        make_range(0, 0, 20, 0),
        Some(TypeInfo::Callable {
            params: vec![],
            ret: Box::new(TypeInfo::Primitive("None".to_string())),
        }),
        None,
        0,
    );
    // Add a non-definition reference
    table.add_reference(sym_id, make_range(25, 0, 25, 7));

    let diag = make_diagnostic(5, DiagnosticSeverity::Error, "E100", "bad call");

    let results = vec![make_file_result(
        "/project/app.py",
        Language::Python,
        vec![diag],
    )];
    let symbol_tables = vec![(PathBuf::from("/project/app.py"), table)];
    let graph = ImportGraph::new();

    let output = builder.build(&results, &symbol_tables, &graph);

    // Symbols
    assert_eq!(output.symbols.len(), 1);
    let sym = output.symbols.get("app.handler").unwrap();
    assert_eq!(sym.kind, "function");
    assert_eq!(sym.type_sig.as_deref(), Some("() -> None"));

    // Issues
    assert_eq!(output.issues.len(), 1);
    assert_eq!(output.issues[0].symbol, "handler");

    // Impact
    assert!(output.impact.contains_key("app.handler"));

    // Stats
    assert_eq!(output.stats.files_checked, 1);
    assert_eq!(output.stats.symbols_found, 1);
    assert_eq!(output.stats.issues_count, 1);
    assert!(output.stats.impact_edges > 0);
}
