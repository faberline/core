use super::*;

/// R2: Files analyzed in dependency order — leaf modules first.
#[test]
fn test_propagation_topological_order() {
    let mut inf = DeepTypeInferencer::new();
    let c = PathBuf::from("c.py");
    let b = PathBuf::from("b.py");
    let a = PathBuf::from("a.py");
    inf.add_file(c.clone());
    inf.add_file(b.clone());
    inf.add_file(a.clone());

    // a → b → c
    inf.add_import_edge(a.clone(), b.clone());
    inf.add_import_edge(b.clone(), c.clone());

    let topo = inf.topological_sort();
    let c_pos = topo.iter().position(|p| p == &c);
    let b_pos = topo.iter().position(|p| p == &b);
    let a_pos = topo.iter().position(|p| p == &a);

    // c should come before b, b before a
    if let (Some(cp), Some(bp), Some(ap)) = (c_pos, b_pos, a_pos) {
        assert!(cp < bp, "c must be analyzed before b");
        assert!(bp < ap, "b must be analyzed before a");
    }
}

/// R1, R2, S3: Transitive propagation A→B→C.
#[test]
fn test_transitive_propagation() {
    let mut inf = DeepTypeInferencer::new();
    let c = PathBuf::from("c.py");
    let b = PathBuf::from("b.py");
    let a = PathBuf::from("a.py");
    inf.add_file(c.clone());
    inf.add_file(b.clone());
    inf.add_file(a.clone());

    // c.py defines Config
    inf.add_file_symbol(
        &c,
        "Config".to_string(),
        binding(
            "Config",
            Type::Instance {
                name: "Config".to_string(),
                module: None,
                type_args: vec![],
            },
            "c.py",
            true,
        ),
    );

    // b.py: from c import Config
    inf.add_import(
        &b,
        ImportInfo {
            module: "c".to_string(),
            names: Some(vec!["Config".to_string()]),
            alias: None,
        },
    );
    inf.add_import_edge(b.clone(), c.clone());

    // a.py: from b import Config
    inf.add_import(
        &a,
        ImportInfo {
            module: "b".to_string(),
            names: Some(vec!["Config".to_string()]),
            alias: None,
        },
    );
    inf.add_import_edge(a.clone(), b.clone());

    let ig = ImportGraph::new();
    let request = PropagationRequest {
        files: vec![c, b, a.clone()],
        changed_files: vec![],
    };
    PropagationPipeline::run(&request, &mut inf, &ig);

    // a.py should have Config with the original type.
    let fa = inf.file_analysis(&a).unwrap();
    let config = fa
        .symbols
        .get("Config")
        .expect("Config should be propagated to a.py");
    assert!(config.is_propagated);
    match &config.ty {
        Type::Instance { name, .. } => assert_eq!(name, "Config"),
        other => panic!("Expected Instance(Config), got {:?}", other),
    }
}

/// R7, S5: When db.pyi exists alongside db.py, propagation uses stub types.
#[test]
fn test_stub_file_preference() {
    let mut inf = DeepTypeInferencer::new();
    let db_py = PathBuf::from("db.py");
    let db_pyi = PathBuf::from("db.pyi");
    let handler = PathBuf::from("handler.py");
    inf.add_file(db_py.clone());
    inf.add_file(db_pyi.clone());
    inf.add_file(handler.clone());

    // db.py has get_user -> Unknown
    inf.add_file_symbol(
        &db_py,
        "get_user".to_string(),
        binding("get_user", Type::Unknown, "db.py", true),
    );
    // db.pyi has get_user -> Int (richer type)
    inf.add_file_symbol(
        &db_pyi,
        "get_user".to_string(),
        binding("get_user", Type::Int, "db.pyi", true),
    );

    // handler.py imports from db
    inf.add_import(
        &handler,
        ImportInfo {
            module: "db".to_string(),
            names: Some(vec!["get_user".to_string()]),
            alias: None,
        },
    );
    inf.add_import_edge(handler.clone(), db_py.clone());

    let ig = ImportGraph::new();
    let request = PropagationRequest {
        files: vec![db_py, db_pyi, handler.clone()],
        changed_files: vec![],
    };
    let result = PropagationPipeline::run(&request, &mut inf, &ig);

    // handler.py should get the stub type (Int), not Unknown.
    let fa = inf.file_analysis(&handler).unwrap();
    let b = fa.symbols.get("get_user").unwrap();
    assert_eq!(b.ty, Type::Int, "Stub type should be preferred");
    assert!(result.stats.stubs_used > 0);
}
