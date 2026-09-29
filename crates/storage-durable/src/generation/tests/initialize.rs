use super::*;

#[test]
fn initialize_empty_refuses_existing_legacy_generation() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("gen-1")).unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    let error = store.initialize_empty().unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.io_error().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::Missing
    );
}

#[test]
fn legacy_adoption_requires_exact_caller_selected_name() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("gen-2")).unwrap();
    std::fs::write(directory.path().join("gen-2/data"), b"two").unwrap();
    std::fs::create_dir(directory.path().join("gen-99")).unwrap();
    std::fs::write(directory.path().join("gen-99/data"), b"ninety-nine").unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    store.adopt_legacy(name("gen-2")).unwrap();
    assert_eq!(current_generation(&store), "gen-2");
}

#[test]
fn legacy_adoption_never_selects_highest_generation() {
    let directory = tempfile::tempdir().unwrap();
    for generation in ["gen-1", "gen-1000"] {
        std::fs::create_dir(directory.path().join(generation)).unwrap();
        std::fs::write(directory.path().join(generation).join("data"), generation).unwrap();
    }
    let store = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::Missing
    );
    store.adopt_legacy(name("gen-1")).unwrap();
    assert_eq!(current_generation(&store), "gen-1");
}

#[test]
fn legacy_adoption_uses_the_same_pointer_commit_semantics() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("gen-1")).unwrap();
    std::fs::write(directory.path().join("gen-1/data"), b"one").unwrap();
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::RenameCurrent,
            occurrence: 0,
            relative_path: PathBuf::from(CURRENT_FILE_NAME),
        },
        io::ErrorKind::Other,
    );
    let error = store.adopt_legacy(name("gen-1")).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::Missing
    );

    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("gen-1")).unwrap();
    std::fs::write(directory.path().join("gen-1/data"), b"one").unwrap();
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncRootAfterCurrent,
            occurrence: 0,
            relative_path: PathBuf::from("."),
        },
        io::ErrorKind::Other,
    );
    let error = store.adopt_legacy(name("gen-1")).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::CommitUncertain);
    assert_eq!(current_generation(&store), "gen-1");
}
