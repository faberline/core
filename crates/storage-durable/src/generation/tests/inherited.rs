use super::*;

#[cfg(unix)]
fn inherited_fixture() -> (
    tempfile::TempDir,
    Arc<RecordingInjector>,
    GenerationStore,
    CurrentGenerationStaging,
) {
    let directory = tempfile::tempdir().unwrap();
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    store.initialize_empty().unwrap();
    let old = store.begin(name("old")).unwrap();
    std::fs::write(old.path().join("payload"), b"old").unwrap();
    store.commit(old).unwrap();
    injector.clear();
    let mut staged = store
        .begin_from_current_if_durable(name("new"))
        .unwrap()
        .expect("successful generic commit must mint durable-current proof");
    std::fs::hard_link(
        directory.path().join("old/payload"),
        staged.path().join("payload"),
    )
    .unwrap();
    staged.inherit_current_file("payload").unwrap();
    std::fs::write(staged.path().join("fresh"), b"fresh").unwrap();
    (directory, injector, store, staged)
}

#[cfg(unix)]
#[test]
fn inherited_begin_requires_process_owned_current_proof() {
    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    assert!(store
        .begin_from_current_if_durable(name("new"))
        .unwrap()
        .is_none());
    assert_eq!(
        store.adopt_legacy(name("old")).unwrap_err().class(),
        CommitFailureClass::PreCommit
    );
    assert!(store
        .begin_from_current_if_durable(name("new"))
        .unwrap()
        .is_none());
    assert!(!directory.path().join(".stage-new").exists());
    assert!(!injector
        .points()
        .iter()
        .any(|point| point.step == CommitStep::SyncFile));

    let empty = tempfile::tempdir().unwrap();
    let empty_store = GenerationStore::open(empty.path()).unwrap();
    empty_store.initialize_empty().unwrap();
    assert_eq!(
        empty_store
            .begin_from_current_if_durable(name("empty"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(!empty.path().join(".stage-empty").exists());

    let (_directory, _injector, store, _staged) = inherited_fixture();
    let second = GenerationStore::open(store.inner.root.clone()).unwrap();
    assert!(second
        .begin_from_current_if_durable(name("second"))
        .unwrap()
        .is_some());
}

#[cfg(unix)]
#[test]
fn inherited_current_proof_expires_after_handles_drop_or_uncertain_commit() {
    let directory = tempfile::tempdir().unwrap();
    {
        let store = GenerationStore::open(directory.path()).unwrap();
        store.initialize_empty().unwrap();
        let old = store.begin(name("old")).unwrap();
        std::fs::write(old.path().join("payload"), b"old").unwrap();
        store.commit(old).unwrap();
        assert!(store
            .begin_from_current_if_durable(name("in-process"))
            .unwrap()
            .is_some());
    }
    let reopened = GenerationStore::open(directory.path()).unwrap();
    assert!(reopened
        .begin_from_current_if_durable(name("after-reopen"))
        .unwrap()
        .is_none());

    let (_directory, injector, store, staged) = inherited_fixture();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncRootAfterCurrent,
            occurrence: 0,
            relative_path: PathBuf::from("."),
        },
        io::ErrorKind::Other,
    );
    assert_eq!(
        store.commit_from_current(staged).unwrap_err().class(),
        CommitFailureClass::CommitUncertain
    );
    assert!(store
        .begin_from_current_if_durable(name("after-uncertain"))
        .unwrap()
        .is_none());

    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    let staged = store.begin(name("failed")).unwrap();
    std::fs::write(staged.path().join("fresh"), b"fresh").unwrap();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncFile,
            occurrence: 0,
            relative_path: PathBuf::from("fresh"),
        },
        io::ErrorKind::Other,
    );
    assert_eq!(
        store.commit(staged).unwrap_err().class(),
        CommitFailureClass::PreCommit
    );
    assert!(store
        .begin_from_current_if_durable(name("after-failed"))
        .unwrap()
        .is_none());
}

#[cfg(unix)]
#[test]
fn inherited_commit_skips_only_registered_current_hard_links() {
    let (_directory, injector, store, staged) = inherited_fixture();
    store.commit_from_current(staged).unwrap();
    let synced: Vec<_> = injector
        .points()
        .into_iter()
        .filter(|point| point.step == CommitStep::SyncFile)
        .map(|point| point.relative_path)
        .collect();
    assert_eq!(synced, vec![PathBuf::from("fresh")]);
    let points = injector.points();
    assert!(points
        .iter()
        .any(|point| point.step == CommitStep::SyncDirectory));
    assert!(points
        .iter()
        .any(|point| point.step == CommitStep::SyncRootAfterGeneration));
    assert!(points
        .iter()
        .any(|point| point.step == CommitStep::SyncCurrentTemp));
    assert!(points
        .iter()
        .any(|point| point.step == CommitStep::SyncRootAfterCurrent));
}

#[cfg(unix)]
#[test]
fn inherited_registration_rejects_copies_bad_paths_and_symlinks() {
    use std::os::unix::fs::symlink;

    let (directory, _injector, store, _existing) = inherited_fixture();
    let mut staged = store
        .begin_from_current_if_durable(name("copy"))
        .unwrap()
        .expect("durable predecessor must permit a second derived stage");
    std::fs::copy(
        directory.path().join("old/payload"),
        staged.path().join("payload"),
    )
    .unwrap();
    assert_eq!(
        staged.inherit_current_file("payload").unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    for bad in [
        "",
        ".",
        "../payload",
        "/payload",
        "nested/./file",
        "nested//file",
        "payload/",
    ] {
        assert_eq!(
            staged.inherit_current_file(bad).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    std::fs::remove_file(staged.path().join("payload")).unwrap();
    symlink(
        directory.path().join("old/payload"),
        staged.path().join("payload"),
    )
    .unwrap();
    assert_eq!(
        staged.inherit_current_file("payload").unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );

    std::fs::create_dir(directory.path().join("old/nested")).unwrap();
    std::fs::hard_link(
        directory.path().join("old/payload"),
        directory.path().join("old/nested/file"),
    )
    .unwrap();
    symlink(
        directory.path().join("old/nested"),
        staged.path().join("nested"),
    )
    .unwrap();
    assert_eq!(
        staged
            .inherit_current_file("nested/file")
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[cfg(unix)]
#[test]
fn inherited_commit_rejects_replaced_or_mutated_or_stale_current_proof() {
    let (directory, _injector, store, staged) = inherited_fixture();
    std::fs::remove_file(staged.path().join("payload")).unwrap();
    std::fs::write(staged.path().join("payload"), b"replacement").unwrap();
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::ValidateStaging);
    assert_eq!(current_generation(&store), "old");

    let (_directory, _injector, store, staged) = inherited_fixture();
    std::fs::write(staged.path().join("payload"), b"changed-in-place").unwrap();
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::ValidateStaging);
    assert_eq!(current_generation(&store), "old");

    let (_directory, _injector, store, staged) = inherited_fixture();
    let replacement = stage_fixture(&store, "other");
    store.commit(replacement).unwrap();
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::ValidateCurrent);
    assert_eq!(current_generation(&store), "other");

    let (_directory, _injector, store, staged) = inherited_fixture();
    let foreign = tempfile::tempdir().unwrap();
    seed_current(foreign.path(), "old");
    let foreign_store = GenerationStore::open(foreign.path()).unwrap();
    let error = foreign_store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::ValidateStaging);
    assert_eq!(current_generation(&store), "old");
    assert_eq!(current_generation(&foreign_store), "old");
    drop(directory);
}

#[cfg(unix)]
#[test]
fn inherited_commit_keeps_fresh_and_pointer_failure_classes() {
    let (_directory, injector, store, staged) = inherited_fixture();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncFile,
            occurrence: 0,
            relative_path: PathBuf::from("fresh"),
        },
        io::ErrorKind::Other,
    );
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::SyncFile);
    assert_eq!(current_generation(&store), "old");

    let (_directory, injector, store, staged) = inherited_fixture();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncDirectory,
            occurrence: 0,
            relative_path: PathBuf::from("."),
        },
        io::ErrorKind::Other,
    );
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::SyncDirectory);
    assert_eq!(current_generation(&store), "old");

    let (_directory, injector, store, staged) = inherited_fixture();
    injector.fail_at(
        FailurePoint {
            step: CommitStep::SyncRootAfterCurrent,
            occurrence: 0,
            relative_path: PathBuf::from("."),
        },
        io::ErrorKind::Other,
    );
    let error = store.commit_from_current(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::CommitUncertain);
    assert_eq!(error.step(), CommitStep::SyncRootAfterCurrent);
    assert_eq!(current_generation(&store), "new");
}

#[cfg(unix)]
#[test]
fn inherited_commit_holds_publication_guard_through_current() {
    let (_directory, injector, store, staged) = inherited_fixture();
    struct Guard(GenerationStore, GenerationName);
    impl Drop for Guard {
        fn drop(&mut self) {
            assert_eq!(
                self.0.read_current().unwrap(),
                CurrentTarget::Generation(self.1.clone())
            );
        }
    }
    let target = staged.generation().clone();
    store
        .commit_from_current_with_publication_guard(staged, || {
            assert!(injector
                .points()
                .iter()
                .any(|point| point.step == CommitStep::SyncRootAfterGeneration));
            assert!(!injector
                .points()
                .iter()
                .any(|point| point.step == CommitStep::WriteCurrentTemp));
            Ok(Guard(store.clone(), target))
        })
        .unwrap();
}
