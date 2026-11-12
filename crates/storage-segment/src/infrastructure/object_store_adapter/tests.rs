use std::sync::Arc;

use storage_object::LocalObjectStore;

use super::ObjectStoreAdapter;
use crate::domain::{ImmutableObjectStore, SegmentError};

#[test]
fn marker_port_is_create_once_and_replace_by_version() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let adapter = ObjectStoreAdapter::new(store);

    assert!(adapter.create_marker("k", b"1").unwrap());
    assert!(!adapter.create_marker("k", b"2").unwrap());

    let (bytes, version) = adapter.read_marker("k").unwrap().unwrap();
    assert_eq!(bytes, b"1".to_vec());
    assert!(matches!(
        adapter.replace_marker("k", b"3", "wrong-version"),
        Err(SegmentError::StagingConflict { .. })
    ));

    adapter.replace_marker("k", b"3", &version).unwrap();
    assert_eq!(adapter.read_marker("k").unwrap().unwrap().0, b"3".to_vec());
}
