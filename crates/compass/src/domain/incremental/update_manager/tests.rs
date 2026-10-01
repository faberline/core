use super::*;

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

// -----------------------------------------------------------------------
// DirtyFileTracker
// -----------------------------------------------------------------------

#[test]
fn test_mark_and_drain() {
    let mut tracker = DirtyFileTracker::new();
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Modified);
    tracker.mark_dirty(p("b.rs"), FileChangeKind::Created);

    assert!(tracker.has_dirty());
    assert_eq!(tracker.dirty_count(), 2);

    let drained = tracker.drain();
    assert_eq!(drained.len(), 2);
    assert!(!tracker.has_dirty());
}

#[test]
fn test_deleted_wins_over_modified() {
    let mut tracker = DirtyFileTracker::new();
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Modified);
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Deleted);

    let drained = tracker.drain();
    assert_eq!(*drained.get(&p("a.rs")).unwrap(), FileChangeKind::Deleted);
}

#[test]
fn test_modified_wins_over_created() {
    let mut tracker = DirtyFileTracker::new();
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Created);
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Modified);

    let drained = tracker.drain();
    assert_eq!(*drained.get(&p("a.rs")).unwrap(), FileChangeKind::Modified);
}

#[test]
fn test_drain_resets_state() {
    let mut tracker = DirtyFileTracker::new();
    tracker.mark_dirty(p("a.rs"), FileChangeKind::Modified);
    tracker.drain();
    assert!(!tracker.has_dirty());
}

// -----------------------------------------------------------------------
// DependencyGraph
// -----------------------------------------------------------------------

#[test]
fn test_add_and_query_edges() {
    let mut graph = DependencyGraph::new();
    graph.add_edge(p("lib.rs"), p("main.rs"));
    graph.add_edge(p("lib.rs"), p("tests.rs"));

    let importers = graph.direct_importers(&p("lib.rs"));
    assert_eq!(importers.len(), 2);
    assert!(importers.contains(&p("main.rs")));
    assert!(importers.contains(&p("tests.rs")));
}

#[test]
fn test_transitive_importers() {
    let mut graph = DependencyGraph::new();
    // a.rs ← b.rs ← c.rs (chain)
    graph.add_edge(p("a.rs"), p("b.rs"));
    graph.add_edge(p("b.rs"), p("c.rs"));

    let affected = graph.transitive_importers(&[p("a.rs")]);
    assert!(affected.contains(&p("a.rs")));
    assert!(affected.contains(&p("b.rs")));
    assert!(affected.contains(&p("c.rs")));
}

#[test]
fn test_remove_file_cleans_edges() {
    let mut graph = DependencyGraph::new();
    graph.add_edge(p("lib.rs"), p("main.rs"));
    graph.remove_file(&p("lib.rs"));

    assert!(graph.direct_importers(&p("lib.rs")).is_empty());
}

#[test]
fn test_no_cycles_in_bfs() {
    let mut graph = DependencyGraph::new();
    // Create a diamond: base ← left/right ← top
    graph.add_edge(p("base.rs"), p("left.rs"));
    graph.add_edge(p("base.rs"), p("right.rs"));
    graph.add_edge(p("left.rs"), p("top.rs"));
    graph.add_edge(p("right.rs"), p("top.rs"));

    let affected = graph.transitive_importers(&[p("base.rs")]);
    // All four should appear exactly once (no duplicates, no infinite loop)
    assert_eq!(affected.len(), 4);
}

// -----------------------------------------------------------------------
// IncrementalUpdateManager
// -----------------------------------------------------------------------

#[test]
fn test_drain_empty_returns_empty() {
    let mut mgr = IncrementalUpdateManager::new();
    assert!(!mgr.has_pending_work());
    assert!(mgr.drain_dirty_files().is_empty());
}

#[test]
fn test_direct_file_change() {
    let mut mgr = IncrementalUpdateManager::new();
    mgr.file_changed(p("src/lib.rs"), FileChangeKind::Modified);

    let files = mgr.drain_dirty_files();
    assert!(files.contains(&p("src/lib.rs")));
}

#[test]
fn test_dependency_aware_expansion() {
    let mut mgr = IncrementalUpdateManager::new();
    // Register imports: main.rs imports lib.rs
    mgr.add_import_edge(p("src/lib.rs"), p("src/main.rs"));

    // lib.rs changes — main.rs should also be flagged
    mgr.file_changed(p("src/lib.rs"), FileChangeKind::Modified);

    let files = mgr.drain_dirty_files();
    assert!(
        files.contains(&p("src/lib.rs")),
        "lib.rs must be in dirty set"
    );
    assert!(
        files.contains(&p("src/main.rs")),
        "main.rs (importer) must be in dirty set"
    );
}

#[test]
fn test_deleted_file_removes_from_graph() {
    let mut mgr = IncrementalUpdateManager::new();
    mgr.add_import_edge(p("lib.rs"), p("main.rs"));
    mgr.file_changed(p("lib.rs"), FileChangeKind::Deleted);

    // After deletion, lib.rs is no longer in the graph
    assert!(mgr.dep_graph().direct_importers(&p("lib.rs")).is_empty());
}

#[test]
fn test_transitive_expansion() {
    let mut mgr = IncrementalUpdateManager::new();
    // chain: a ← b ← c
    mgr.add_import_edge(p("a.rs"), p("b.rs"));
    mgr.add_import_edge(p("b.rs"), p("c.rs"));

    mgr.file_changed(p("a.rs"), FileChangeKind::Modified);
    let files = mgr.drain_dirty_files();

    assert!(files.contains(&p("a.rs")));
    assert!(files.contains(&p("b.rs")));
    assert!(files.contains(&p("c.rs")));
}

#[test]
fn test_drain_clears_tracker() {
    let mut mgr = IncrementalUpdateManager::new();
    mgr.file_changed(p("x.rs"), FileChangeKind::Modified);
    mgr.drain_dirty_files();
    assert!(!mgr.has_pending_work());
}

#[test]
fn test_result_is_sorted() {
    let mut mgr = IncrementalUpdateManager::new();
    mgr.file_changed(p("z.rs"), FileChangeKind::Modified);
    mgr.file_changed(p("a.rs"), FileChangeKind::Modified);
    mgr.file_changed(p("m.rs"), FileChangeKind::Modified);

    let files = mgr.drain_dirty_files();
    let sorted = {
        let mut s = files.clone();
        s.sort();
        s
    };
    assert_eq!(files, sorted);
}
