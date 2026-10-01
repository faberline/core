use super::*;

#[test]
fn test_change_tracker() {
    let mut tracker = ChangeTracker::new().with_debounce(Duration::ZERO);

    tracker.record_change(
        PathBuf::from("test.py"),
        ChangeKind::Modified,
        ContentHash::from_content("abc"),
    );

    let changes = tracker.get_pending_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file, PathBuf::from("test.py"));
}

#[test]
fn test_dependency_graph() {
    let mut graph = DependencyGraph::new();

    graph.add_dependency(PathBuf::from("a.py"), PathBuf::from("b.py"));
    graph.add_dependency(PathBuf::from("b.py"), PathBuf::from("c.py"));

    let affected = graph.get_affected_files(&PathBuf::from("c.py"));
    assert!(affected.contains(&PathBuf::from("b.py")));
    assert!(affected.contains(&PathBuf::from("a.py")));
}

#[test]
fn test_incremental_analyzer() {
    let mut analyzer = IncrementalAnalyzer::default_config();

    // Mark file as changed
    let file = PathBuf::from("test.py");
    analyzer.file_changed(
        file.clone(),
        ChangeKind::Modified,
        ContentHash::from_content("abc"),
    );

    // Wait for debounce period to pass
    std::thread::sleep(Duration::from_millis(350));

    let files = analyzer.get_files_to_analyze();
    assert!(
        !files.is_empty(),
        "Should have files to analyze after debounce"
    );
    assert!(files.contains(&file), "Should include the changed file");
}
