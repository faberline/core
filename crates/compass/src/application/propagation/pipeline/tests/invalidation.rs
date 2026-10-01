use super::*;

/// R8, S6: After invalidation + re-propagation, importers get updated types.
#[test]
fn test_repropagate_after_invalidation() {
    let (mut inf, ig) = setup_two_file();
    let db = PathBuf::from("db.py");
    let handler = PathBuf::from("handler.py");

    // Initial propagation.
    let request = PropagationRequest {
        files: vec![db.clone(), handler.clone()],
        changed_files: vec![],
    };
    PropagationPipeline::run(&request, &mut inf, &ig);

    // Now change db.py: get_user now returns Str instead.
    inf.add_file_symbol(
        &db,
        "get_user".to_string(),
        binding("get_user", Type::Str, "db.py", true),
    );

    // Invalidate and re-propagate.
    PropagationPipeline::invalidate_and_repropagate(&db, &mut inf, &ig);

    // handler.py should have the updated type.
    let fa = inf.file_analysis(&handler).unwrap();
    let b = fa.symbols.get("get_user").unwrap();
    assert_eq!(
        b.ty,
        Type::Str,
        "handler.py should have updated Str type after re-propagation"
    );
}

/// S7: File with no project imports — propagation is skipped.
#[test]
fn test_no_imports_no_propagation() {
    let mut inf = DeepTypeInferencer::new();
    let solo = PathBuf::from("solo.py");
    inf.add_file(solo.clone());
    inf.add_file_symbol(
        &solo,
        "x".to_string(),
        binding("x", Type::Int, "solo.py", true),
    );

    let ig = ImportGraph::new();
    let request = PropagationRequest {
        files: vec![solo.clone()],
        changed_files: vec![],
    };
    let result = PropagationPipeline::run(&request, &mut inf, &ig);

    assert_eq!(
        result.stats.symbols_propagated, 0,
        "No imports means no propagation"
    );
    assert!(result.propagated.is_empty());
    // File should still be marked complete.
    let fa = inf.file_analysis(&solo).unwrap();
    assert!(fa.propagation_complete);
}

/// Schema: PropagationResult.stats correctly counts files_analyzed,
/// symbols_propagated, cycles_detected.
#[test]
fn test_propagation_stats() {
    let (mut inf, ig) = setup_two_file();

    let request = PropagationRequest {
        files: vec![PathBuf::from("db.py"), PathBuf::from("handler.py")],
        changed_files: vec![],
    };
    let result = PropagationPipeline::run(&request, &mut inf, &ig);

    assert_eq!(result.stats.files_analyzed, 2);
    assert!(result.stats.symbols_propagated >= 1);
    assert_eq!(result.stats.cycles_detected, 0);
    assert!(result.stats.time_ms < 10_000, "Should finish in < 10s");
}
