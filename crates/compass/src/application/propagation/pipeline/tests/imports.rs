use super::*;

// -- Spec-required unit tests -------------------------------------------

/// R1, R6, S1: `from db import get_user` propagates Callable type.
#[test]
fn test_propagate_from_import_y() {
    let (mut inf, ig) = setup_two_file();
    let handler = PathBuf::from("handler.py");

    let request = PropagationRequest {
        files: vec![PathBuf::from("db.py"), handler.clone()],
        changed_files: vec![],
    };

    let result = PropagationPipeline::run(&request, &mut inf, &ig);

    // handler.py should now have a propagated get_user binding.
    let fa = inf.file_analysis(&handler).unwrap();
    let b = fa.symbols.get("get_user").unwrap();
    assert!(b.is_propagated, "get_user should be marked as propagated");
    match &b.ty {
        Type::Callable { ret, .. } => match ret.as_ref() {
            Type::Instance { name, .. } => assert_eq!(name, "User"),
            other => panic!("Expected Instance(User), got {:?}", other),
        },
        other => panic!("Expected Callable, got {:?}", other),
    }
    assert!(result.stats.symbols_propagated > 0);
}

/// R6, S2: `import db` propagates all exported symbols as module-level binding.
#[test]
fn test_propagate_import_module() {
    let mut inf = DeepTypeInferencer::new();
    let db = PathBuf::from("db.py");
    let handler = PathBuf::from("handler.py");
    inf.add_file(db.clone());
    inf.add_file(handler.clone());

    // db.py exports two symbols
    inf.add_file_symbol(
        &db,
        "get_user".to_string(),
        binding("get_user", Type::Int, "db.py", true),
    );
    inf.add_file_symbol(
        &db,
        "create_user".to_string(),
        binding("create_user", Type::Str, "db.py", true),
    );

    // handler.py: import db (no specific names → all exported)
    inf.add_import(
        &handler,
        ImportInfo {
            module: "db".to_string(),
            names: None,
            alias: None,
        },
    );
    inf.add_import_edge(handler.clone(), db.clone());

    let ig = ImportGraph::new();
    let request = PropagationRequest {
        files: vec![db, handler.clone()],
        changed_files: vec![],
    };
    PropagationPipeline::run(&request, &mut inf, &ig);

    let fa = inf.file_analysis(&handler).unwrap();
    // Both exported symbols should be propagated.
    assert!(fa.symbols.contains_key("get_user"));
    assert!(fa.symbols.contains_key("create_user"));
}

/// R6: `from db import *` propagates all non-underscore symbols.
#[test]
fn test_star_import_propagation() {
    let mut inf = DeepTypeInferencer::new();
    let db = PathBuf::from("db.py");
    let handler = PathBuf::from("handler.py");
    inf.add_file(db.clone());
    inf.add_file(handler.clone());

    // db.py exports two symbols, one private
    inf.add_file_symbol(
        &db,
        "get_user".to_string(),
        binding("get_user", Type::Int, "db.py", true),
    );
    inf.add_file_symbol(
        &db,
        "create_user".to_string(),
        binding("create_user", Type::Str, "db.py", true),
    );
    inf.add_file_symbol(
        &db,
        "_internal".to_string(),
        binding("_internal", Type::Float, "db.py", false),
    );

    // handler.py: from db import * (no specific names = all exported)
    inf.add_import(
        &handler,
        ImportInfo {
            module: "db".to_string(),
            names: None, // wildcard
            alias: None,
        },
    );
    inf.add_import_edge(handler.clone(), db.clone());

    let ig = ImportGraph::new();
    let request = PropagationRequest {
        files: vec![db, handler.clone()],
        changed_files: vec![],
    };
    PropagationPipeline::run(&request, &mut inf, &ig);

    let fa = inf.file_analysis(&handler).unwrap();
    // Exported symbols should be propagated.
    assert!(fa.symbols.contains_key("get_user"));
    assert!(fa.symbols.contains_key("create_user"));
    // Non-exported should NOT be propagated.
    assert!(
        !fa.symbols.contains_key("_internal"),
        "_internal is not exported and should not propagate"
    );
}
