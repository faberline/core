use super::*;

#[test]
fn commit_syncs_all_files_then_directories_leaf_to_root() {
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    store.commit(staged).unwrap();
    let points = injector.points();
    let sync_points: Vec<_> = points
        .iter()
        .filter(|point| matches!(point.step, CommitStep::SyncFile | CommitStep::SyncDirectory))
        .map(|point| (point.step, point.occurrence, point.relative_path.clone()))
        .collect();
    assert_eq!(
        sync_points,
        vec![
            (CommitStep::SyncFile, 0, PathBuf::from("a.txt")),
            (CommitStep::SyncFile, 1, PathBuf::from("b/c/n.txt")),
            (CommitStep::SyncFile, 2, PathBuf::from("b/m.txt")),
            (CommitStep::SyncFile, 3, PathBuf::from("z.txt")),
            (CommitStep::SyncDirectory, 0, PathBuf::from("b/c")),
            (CommitStep::SyncDirectory, 1, PathBuf::from("a-dir")),
            (CommitStep::SyncDirectory, 2, PathBuf::from("b")),
            (CommitStep::SyncDirectory, 3, PathBuf::from(".")),
        ]
    );
}

#[test]
fn publication_guard_starts_after_file_sync_and_lasts_through_current() {
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    struct Guard(GenerationStore, GenerationName);
    impl Drop for Guard {
        fn drop(&mut self) {
            assert_eq!(
                self.0.read_current().unwrap(),
                CurrentTarget::Generation(self.1.clone())
            );
        }
    }
    let target = staged.generation.clone();
    store
        .commit_with_publication_guard(staged, || {
            let points = injector.points();
            assert!(
                points
                    .iter()
                    .any(|point| point.step == CommitStep::SyncRootAfterGeneration),
                "publication guard acquired before generation files became durable"
            );
            assert!(!points
                .iter()
                .any(|point| point.step == CommitStep::WriteCurrentTemp));
            Ok(Guard(store.clone(), target))
        })
        .unwrap();
}

#[test]
fn commit_renames_generation_before_writing_current() {
    let points = successful_points(false);
    let rename_generation = points
        .iter()
        .position(|point| point.step == CommitStep::RenameGeneration)
        .unwrap();
    let write_current = points
        .iter()
        .position(|point| point.step == CommitStep::WriteCurrentTemp)
        .unwrap();
    assert!(rename_generation < write_current);
}

#[test]
fn current_rename_is_the_only_commit_point() {
    let rename = successful_points(false)
        .into_iter()
        .find(|point| point.step == CommitStep::RenameCurrent)
        .unwrap();
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    injector.fail_at(rename, io::ErrorKind::Other);
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(current_generation(&store), "old");

    let final_sync = successful_points(false)
        .into_iter()
        .find(|point| point.step == CommitStep::SyncRootAfterCurrent)
        .unwrap();
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    injector.fail_at(final_sync, io::ErrorKind::Other);
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::CommitUncertain);
    assert_eq!(current_generation(&store), "new");
}

#[test]
fn every_precommit_injection_preserves_old_current() {
    let points = successful_points(true);
    for point in points
        .into_iter()
        .filter(|point| point.step != CommitStep::SyncRootAfterCurrent)
    {
        let (_directory, injector, store, staged) = instrumented_fixture(true);
        injector.fail_at(point.clone(), io::ErrorKind::Other);
        let error = store.commit(staged).unwrap_err();
        assert_eq!(
            error.class(),
            CommitFailureClass::PreCommit,
            "unexpected class at {point:?}"
        );
        assert_eq!(
            current_generation(&store),
            "old",
            "CURRENT changed at {point:?}"
        );
    }
}

#[test]
fn final_root_sync_failure_is_commit_uncertain() {
    let point = successful_points(false)
        .into_iter()
        .find(|point| point.step == CommitStep::SyncRootAfterCurrent)
        .unwrap();
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    injector.fail_at(point, io::ErrorKind::Other);
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::CommitUncertain);
    assert_eq!(error.step(), CommitStep::SyncRootAfterCurrent);
    assert_eq!(current_generation(&store), "new");
}

#[test]
fn precommit_storage_full_preserves_io_kind_and_old_current() {
    let point = successful_points(false)
        .into_iter()
        .find(|point| point.step == CommitStep::WriteCurrentTemp)
        .unwrap();
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    injector.fail_at(point, io::ErrorKind::StorageFull);
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.io_error().kind(), io::ErrorKind::StorageFull);
    assert_eq!(current_generation(&store), "old");
}

#[test]
fn commit_uncertain_storage_full_preserves_io_kind() {
    let point = successful_points(false)
        .into_iter()
        .find(|point| point.step == CommitStep::SyncRootAfterCurrent)
        .unwrap();
    let (_directory, injector, store, staged) = instrumented_fixture(false);
    injector.fail_at(point, io::ErrorKind::StorageFull);
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::CommitUncertain);
    assert_eq!(error.io_error().kind(), io::ErrorKind::StorageFull);
}

#[cfg(unix)]
#[test]
fn commit_rejects_symlink_and_special_staging_entries() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    let store = GenerationStore::open(directory.path()).unwrap();
    let staged = store.begin(name("symlinked")).unwrap();
    symlink(
        directory.path().join("old/payload"),
        staged.path().join("payload-link"),
    )
    .unwrap();
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(current_generation(&store), "old");

    let staged = store.begin(name("socketed")).unwrap();
    let _listener = UnixListener::bind(staged.path().join("socket")).unwrap();
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(current_generation(&store), "old");
}

#[test]
fn commit_refuses_existing_immutable_generation() {
    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    let store = GenerationStore::open(directory.path()).unwrap();
    let staged = store.begin(name("new")).unwrap();
    std::fs::create_dir(directory.path().join("new")).unwrap();
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(error.step(), CommitStep::ValidateStaging);
    assert_eq!(current_generation(&store), "old");
}

#[test]
fn stale_regular_current_tmp_is_removed_safely() {
    let (directory, _injector, store, staged) = instrumented_fixture(true);
    store.commit(staged).unwrap();
    assert_eq!(current_generation(&store), "new");
    assert!(!directory.path().join(CURRENT_TEMP_FILE_NAME).exists());
}

#[cfg(unix)]
#[test]
fn symlink_current_tmp_is_rejected_without_touching_current() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    symlink(
        directory.path().join("old/payload"),
        directory.path().join(CURRENT_TEMP_FILE_NAME),
    )
    .unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    let staged = stage_fixture(&store, "new");
    let error = store.commit(staged).unwrap_err();
    assert_eq!(error.step(), CommitStep::RemoveStaleCurrentTemp);
    assert_eq!(error.class(), CommitFailureClass::PreCommit);
    assert_eq!(current_generation(&store), "old");
    assert!(directory
        .path()
        .join(CURRENT_TEMP_FILE_NAME)
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn failure_points_have_stable_step_occurrence_and_path_order() {
    let first = successful_points(true);
    let second = successful_points(true);
    assert_eq!(first, second);
    for step in [CommitStep::SyncFile, CommitStep::SyncDirectory] {
        let occurrences: Vec<_> = first
            .iter()
            .filter(|point| point.step == step)
            .map(|point| point.occurrence)
            .collect();
        assert_eq!(occurrences, (0..occurrences.len()).collect::<Vec<_>>());
    }
}
