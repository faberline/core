use storage_durable::FsyncPolicy;
use storage_object::{ObjectStore, ObjectStoreError, PutCondition, ScratchObjectStore};

#[test]
fn scratch_store_preserves_bytes_versions_and_conditional_puts() {
    let dir = tempfile::tempdir().unwrap();
    let store = ScratchObjectStore::open(dir.path()).unwrap();
    let first = store
        .put(
            "segments/logs/one.parquet",
            b"first",
            "application/vnd.apache.parquet",
            PutCondition::IfAbsent,
        )
        .unwrap();
    assert_eq!(store.get(first.key()).unwrap().bytes(), b"first");
    assert_eq!(store.head(first.key()).unwrap(), first);
    assert!(matches!(
        store.put(first.key(), b"bad", "text/plain", PutCondition::IfAbsent),
        Err(ObjectStoreError::PreconditionFailed { .. })
    ));
    let second = store
        .put(
            first.key(),
            b"second",
            "application/octet-stream",
            PutCondition::IfVersion(first.version().clone()),
        )
        .unwrap();
    assert_ne!(first.version(), second.version());
    assert_eq!(store.list("segments/logs").unwrap(), vec![second.clone()]);
    store.delete(second.key()).unwrap();
    assert!(matches!(
        store.get(second.key()),
        Err(ObjectStoreError::NotFound { .. })
    ));
}

#[test]
fn scratch_store_rejects_path_escape_and_symlink_components() {
    let dir = tempfile::tempdir().unwrap();
    let store = ScratchObjectStore::open(dir.path()).unwrap();
    assert!(matches!(
        store.put("../escape", b"x", "text/plain", PutCondition::Any),
        Err(ObjectStoreError::InvalidKey { .. })
    ));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path(), dir.path().join("link")).unwrap();
        assert!(matches!(
            store.put("link/escape", b"x", "text/plain", PutCondition::Any),
            Err(ObjectStoreError::UnsafePath { .. })
        ));
    }
}

// `atomic_write` in crates/storage-durable/src/atomic.rs skips `sync_all` and
// `sync_parent_dir` for `FsyncPolicy::Os`, and ScratchObjectStore passes its
// own policy (`self.fsync`) to both of its writes.
#[test]
fn scratch_store_never_fsyncs() {
    let dir = tempfile::tempdir().unwrap();
    let store = ScratchObjectStore::open(dir.path()).unwrap();
    assert_eq!(store.fsync_policy(), FsyncPolicy::Os);
    let meta = store
        .put(
            "scratch/one.bin",
            b"scratch",
            "application/octet-stream",
            PutCondition::Any,
        )
        .unwrap();
    assert_eq!(store.get(meta.key()).unwrap().bytes(), b"scratch");
}

#[test]
fn local_store_keeps_always_fsync() {
    let dir = tempfile::tempdir().unwrap();
    let store = storage_object::LocalObjectStore::open(dir.path()).unwrap();
    assert_eq!(store.fsync_policy(), FsyncPolicy::Always);
}
