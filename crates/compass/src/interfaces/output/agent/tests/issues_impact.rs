use super::*;

// --- R4, R6: build_issues with symbol attribution ---

#[test]
fn test_build_issues_with_symbol_attribution() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    // Create a symbol table with a function at lines 10-20 (0-indexed)
    let table = make_symbol_table_with_function("get_user", SymbolKind::Function, 10, 20, None);

    // Create a diagnostic inside the function (line 15, 0-indexed)
    let diag = make_diagnostic(15, DiagnosticSeverity::Error, "PY101", "undefined variable");

    let results = vec![make_file_result(
        "/project/db.py",
        Language::Python,
        vec![diag],
    )];
    let symbol_tables = vec![(PathBuf::from("/project/db.py"), table)];

    let issues = builder.build_issues(&results, &table_views(&symbol_tables));

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert_eq!(issue.severity, "error");
    assert_eq!(
        issue.symbol, "get_user",
        "should be attributed to enclosing symbol"
    );
    assert_eq!(issue.file, "db.py");
    assert_eq!(issue.line, 16); // 0-indexed 15 -> 1-indexed 16
    assert_eq!(issue.code, "PY101");
    assert_eq!(issue.message, "undefined variable");
}

#[test]
fn test_build_issues_file_level_fallback() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    // Symbol table with function at lines 10-20
    let table = make_symbol_table_with_function("main", SymbolKind::Function, 10, 20, None);

    // Diagnostic at line 5 — outside any symbol
    let diag = make_diagnostic(
        5,
        DiagnosticSeverity::Warning,
        "SYN001",
        "missing newline at EOF",
    );

    let results = vec![make_file_result(
        "/project/app.py",
        Language::Python,
        vec![diag],
    )];
    let symbol_tables = vec![(PathBuf::from("/project/app.py"), table)];

    let issues = builder.build_issues(&results, &table_views(&symbol_tables));

    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].symbol, "<file-level>",
        "diagnostic outside any symbol should use <file-level> (R6)"
    );
}

// --- R5: build_impact from references ---

#[test]
fn test_build_impact_from_references() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let mut table = SymbolTable::new();
    // Add a function symbol
    let sym_id = table.add_symbol(
        "get_user".to_string(),
        SymbolKind::Function,
        make_range(10, 0, 20, 0),
        None,
        None,
        0,
    );

    // Add a non-definition reference to get_user at line 30
    table.add_reference(sym_id, make_range(30, 4, 30, 12));
    // Add another reference at line 45
    table.add_reference(sym_id, make_range(45, 8, 45, 16));

    let symbol_tables = vec![(PathBuf::from("/project/db.py"), table)];

    let impact = builder.build_impact(&table_views(&symbol_tables));

    let refs = impact
        .get("db.get_user")
        .expect("should have impact for db.get_user");
    assert_eq!(refs.len(), 2);
    assert!(refs.contains(&"db.py:31".to_string())); // 0-indexed 30 -> 1-indexed 31
    assert!(refs.contains(&"db.py:46".to_string())); // 0-indexed 45 -> 1-indexed 46
}

// --- R8: stats computation ---

#[test]
fn test_stats_computation() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    // Two files
    let mut table1 = SymbolTable::new();
    let sym_id = table1.add_symbol(
        "func_a".to_string(),
        SymbolKind::Function,
        make_range(0, 0, 10, 0),
        None,
        None,
        0,
    );
    table1.add_reference(sym_id, make_range(20, 0, 20, 5)); // non-def ref

    let table2 = make_symbol_table_with_function("func_b", SymbolKind::Function, 0, 5, None);

    let diag = make_diagnostic(3, DiagnosticSeverity::Error, "E001", "error");
    let results = vec![
        make_file_result("/project/a.py", Language::Python, vec![diag]),
        make_file_result("/project/b.py", Language::Python, vec![]),
    ];
    let symbol_tables = vec![
        (PathBuf::from("/project/a.py"), table1),
        (PathBuf::from("/project/b.py"), table2),
    ];

    let graph = ImportGraph::new();
    let output = builder.build(&results, &symbol_tables, &graph);

    assert_eq!(output.stats.files_checked, 2);
    assert_eq!(output.stats.symbols_found, 2);
    assert_eq!(output.stats.issues_count, 1);
    // 1 non-definition reference to func_a
    assert_eq!(output.stats.impact_edges, 1);
}
