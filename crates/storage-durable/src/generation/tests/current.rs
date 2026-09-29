use super::*;

#[test]
fn restart_reads_only_the_named_current_generation() {
    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    std::fs::create_dir(directory.path().join("newer")).unwrap();
    let reopened = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(current_generation(&reopened), "old");
}

#[test]
fn restart_ignores_unpointed_and_staging_generations() {
    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "selected");
    std::fs::create_dir(directory.path().join("zzzz-unpointed")).unwrap();
    std::fs::create_dir(directory.path().join(".stage-future")).unwrap();
    let reopened = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(current_generation(&reopened), "selected");
}

#[test]
fn read_current_returns_missing_instead_of_auto_selecting() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("gen-999")).unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    let error = store.read_current().unwrap_err();
    assert_eq!(error.kind, CurrentReadErrorKind::Missing);
}

#[test]
fn read_current_accepts_explicit_empty() {
    let directory = tempfile::tempdir().unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    store.initialize_empty().unwrap();
    assert_eq!(store.read_current().unwrap(), CurrentTarget::Empty);
    let reopened = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(reopened.read_current().unwrap(), CurrentTarget::Empty);
}

#[test]
fn read_current_rejects_malformed_pointer() {
    for bytes in [
        b"".as_slice(),
        b"empty".as_slice(),
        b"empty\nextra".as_slice(),
        b"generation:gen-1".as_slice(),
        b"generation:gen-1\nextra".as_slice(),
        b"other:gen-1\n".as_slice(),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join(CURRENT_FILE_NAME), bytes).unwrap();
        let store = GenerationStore::open(directory.path()).unwrap();
        assert_eq!(
            store.read_current().unwrap_err().kind,
            CurrentReadErrorKind::Malformed
        );
    }
}

#[test]
fn read_current_rejects_traversal_target() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(CURRENT_FILE_NAME),
        b"generation:../outside\n",
    )
    .unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::UnsafeTarget
    );
}

#[test]
fn read_current_rejects_missing_target() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(CURRENT_FILE_NAME),
        b"generation:missing\n",
    )
    .unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::TargetMissing
    );
}

#[cfg(unix)]
#[test]
fn read_current_rejects_symlink_and_non_directory_target() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("file-target"), b"x").unwrap();
    std::fs::write(
        directory.path().join(CURRENT_FILE_NAME),
        b"generation:file-target\n",
    )
    .unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::TargetNotDirectory
    );

    std::fs::create_dir(directory.path().join("real-target")).unwrap();
    symlink(
        directory.path().join("real-target"),
        directory.path().join("link-target"),
    )
    .unwrap();
    std::fs::write(
        directory.path().join(CURRENT_FILE_NAME),
        b"generation:link-target\n",
    )
    .unwrap();
    assert_eq!(
        store.read_current().unwrap_err().kind,
        CurrentReadErrorKind::TargetSymlink
    );
}
