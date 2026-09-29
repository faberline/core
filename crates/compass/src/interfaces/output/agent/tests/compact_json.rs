use super::*;

// --- R9: compact output omits empty ---

#[test]
fn test_compact_output_omits_empty() {
    // No issues scenario
    let output = AgentOutput {
        symbols: {
            let mut m = BTreeMap::new();
            m.insert(
                "mod.func".to_string(),
                SymbolDef {
                    type_sig: None,
                    file: "mod.py".to_string(),
                    line: 1,
                    kind: "function".to_string(),
                },
            );
            m
        },
        imports: BTreeMap::new(),
        issues: Vec::new(),
        impact: BTreeMap::new(),
        stats: AgentStats {
            files_checked: 1,
            symbols_found: 1,
            issues_count: 0,
            impact_edges: 0,
        },
    };

    let json_str = serde_json::to_string(&output).unwrap();

    // "issues" key should be absent
    assert!(
        !json_str.contains("\"issues\""),
        "issues should be omitted when empty (R9)"
    );
    // "symbols" should always be present
    assert!(json_str.contains("\"symbols\""));
    // "stats" should always be present
    assert!(json_str.contains("\"stats\""));
}

#[test]
fn test_compact_output_omits_empty_imports() {
    let output = AgentOutput {
        symbols: BTreeMap::new(),
        imports: BTreeMap::new(),
        issues: vec![AgentIssue {
            severity: "error".to_string(),
            symbol: "main".to_string(),
            file: "app.py".to_string(),
            line: 1,
            code: "E001".to_string(),
            message: "test".to_string(),
        }],
        impact: BTreeMap::new(),
        stats: AgentStats {
            files_checked: 1,
            symbols_found: 0,
            issues_count: 1,
            impact_edges: 0,
        },
    };

    let json_str = serde_json::to_string(&output).unwrap();

    // "imports" key should be absent
    assert!(
        !json_str.contains("\"imports\""),
        "imports should be omitted when empty (R9)"
    );
    // "impact" key should be absent
    assert!(
        !json_str.contains("\"impact\""),
        "impact should be omitted when empty (R9)"
    );
    // "issues" should be present since non-empty
    assert!(json_str.contains("\"issues\""));
}

// --- NF4: valid JSON round-trip ---

#[test]
fn test_agent_output_valid_json() {
    let root = PathBuf::from("/project");
    let builder = AgentOutputBuilder::new(&root);

    let mut table = SymbolTable::new();
    let sym_id = table.add_symbol(
        "process".to_string(),
        SymbolKind::Function,
        make_range(0, 0, 10, 0),
        Some(TypeInfo::Callable {
            params: vec![TypeInfo::Primitive("str".to_string())],
            ret: Box::new(TypeInfo::Primitive("bool".to_string())),
        }),
        None,
        0,
    );
    table.add_reference(sym_id, make_range(20, 0, 20, 7));

    let diag = make_diagnostic(5, DiagnosticSeverity::Warning, "W001", "unused variable");
    let results = vec![make_file_result(
        "/project/main.py",
        Language::Python,
        vec![diag],
    )];
    let symbol_tables = vec![(PathBuf::from("/project/main.py"), table)];
    let graph = ImportGraph::new();

    let output = builder.build(&results, &symbol_tables, &graph);
    let json_str = serde_json::to_string(&output).unwrap();

    // Verify it's parseable
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .expect("AgentOutput JSON must be parseable by serde_json (NF4)");

    // Verify structure
    assert!(parsed.get("symbols").is_some());
    assert!(parsed.get("stats").is_some());
    assert!(parsed["stats"]["files_checked"].as_u64().unwrap() == 1);
}
