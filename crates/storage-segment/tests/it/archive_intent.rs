use std::sync::Arc;

use storage_object::{LocalObjectStore, ObjectStore, ObjectStoreError, PutCondition};
use storage_segment::{ArchiveCoordinator, ArchiveIntent, ArchiveObject, SegmentError};

const SEGMENT_KEY: &str = "archive/segment-0001.parquet";
const CONTENT_TYPE: &str = "application/vnd.apache.parquet";
const SEGMENT_SHA256: &str = "03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f";

fn setup() -> (tempfile::TempDir, Arc<LocalObjectStore>, ArchiveCoordinator) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let coordinator = ArchiveCoordinator::new(store.clone());
    (dir, store, coordinator)
}

fn segment() -> ArchiveObject {
    ArchiveObject::new(SEGMENT_KEY, b"segment".to_vec(), CONTENT_TYPE)
}

#[test]
fn crash_between_intent_and_upload_leaves_a_pending_intent() {
    let (_dir, store, coordinator) = setup();
    let intent = ArchiveIntent::for_object(&segment());

    coordinator.write_intent(&intent).unwrap();

    assert_eq!(coordinator.pending_intents().unwrap(), vec![intent]);
    assert!(matches!(
        store.head(SEGMENT_KEY),
        Err(ObjectStoreError::NotFound { .. })
    ));
}

#[test]
fn intents_are_listed_in_key_order_and_other_objects_are_ignored() {
    let (_dir, _store, coordinator) = setup();
    let other = ArchiveObject::new("archive/other.parquet", b"segment".to_vec(), CONTENT_TYPE);
    let other_intent = ArchiveIntent::for_object(&other);
    let segment_intent = ArchiveIntent::for_object(&segment());

    coordinator.write_intent(&segment_intent).unwrap();
    coordinator.write_intent(&other_intent).unwrap();
    let mut transaction = coordinator.begin();
    transaction
        .put(ArchiveObject::new(
            "archive/data.bin",
            b"data".to_vec(),
            "application/octet-stream",
        ))
        .unwrap();

    assert_eq!(
        coordinator.pending_intents().unwrap(),
        vec![other_intent, segment_intent]
    );
}

#[test]
fn a_second_write_of_the_same_intent_is_accepted() {
    let (_dir, _store, coordinator) = setup();
    let intent = ArchiveIntent::for_object(&segment());

    coordinator.write_intent(&intent).unwrap();
    coordinator.write_intent(&intent).unwrap();

    assert_eq!(coordinator.pending_intents().unwrap(), vec![intent]);
}

#[test]
fn a_changed_intent_under_the_same_segment_key_is_refused() {
    let (_dir, _store, coordinator) = setup();
    let intent = ArchiveIntent::for_object(&segment());
    let changed = ArchiveIntent::new(SEGMENT_KEY, 8, SEGMENT_SHA256, CONTENT_TYPE);

    coordinator.write_intent(&intent).unwrap();

    assert!(matches!(
        coordinator.write_intent(&changed),
        Err(SegmentError::ImmutableObjectChanged { .. })
    ));
}

#[test]
fn a_corrupt_record_under_the_prefix_is_an_error() {
    let (_dir, store, coordinator) = setup();
    store
        .put(
            "_archive/intent/bad.json",
            b"{",
            "application/json",
            PutCondition::IfAbsent,
        )
        .unwrap();

    assert!(matches!(
        coordinator.pending_intents(),
        Err(SegmentError::CorruptArchiveIntent { .. })
    ));
}
