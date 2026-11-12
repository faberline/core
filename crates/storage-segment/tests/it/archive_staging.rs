use std::sync::Arc;

use storage_object::{LocalObjectStore, ObjectStore, PutCondition};
use storage_segment::{
    ArchiveCoordinator, ArchiveObject, SegmentError, StagingMarker, StagingState,
};

const KEY: &str = "archive/segment-0001.parquet";
const MARKER_KEY: &str =
    "_archive/stage/fb6c3989579d2eae9aff55014178ae1764a8ef8b1ad93c0294d46d8ce7fac5b2.json";
const INTENT_TEXT: &str =
    r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"intent"}"#;
const UPLOADED_TEXT: &str =
    r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"uploaded"}"#;

fn setup() -> (tempfile::TempDir, Arc<LocalObjectStore>, ArchiveCoordinator) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let coordinator = ArchiveCoordinator::new(store.clone());
    (dir, store, coordinator)
}

fn read_marker_text(dir: &tempfile::TempDir) -> String {
    let bytes = std::fs::read(dir.path().join(MARKER_KEY)).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn begin_staging_writes_an_intent_marker_the_store_lists() {
    let (_dir, store, coordinator) = setup();

    let marker = coordinator.begin_staging(KEY).unwrap();

    assert_eq!(marker.state, StagingState::Intent);
    let listed = store.list("_archive/stage/").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key(), MARKER_KEY);
}

#[test]
fn marker_bytes_are_pinned() {
    let (dir, _store, coordinator) = setup();

    coordinator.begin_staging(KEY).unwrap();
    assert_eq!(read_marker_text(&dir), INTENT_TEXT);

    coordinator
        .advance_staging(KEY, StagingState::Uploaded)
        .unwrap();
    assert_eq!(read_marker_text(&dir), UPLOADED_TEXT);
}

#[test]
fn full_path_intent_uploaded_committed() {
    let (_dir, _store, coordinator) = setup();

    coordinator.begin_staging(KEY).unwrap();
    let uploaded = coordinator
        .advance_staging(KEY, StagingState::Uploaded)
        .unwrap();
    let committed = coordinator
        .advance_staging(KEY, StagingState::Committed)
        .unwrap();

    assert_eq!(uploaded.state, StagingState::Uploaded);
    assert_eq!(committed.state, StagingState::Committed);
    let stored = coordinator.staging_marker(KEY).unwrap().unwrap();
    assert_eq!(stored.state, StagingState::Committed);
}

#[test]
fn crash_between_intent_and_upload_resumes_without_a_second_intent() {
    let (_dir, store, coordinator) = setup();
    coordinator.begin_staging(KEY).unwrap();

    // The crash: the coordinator is dropped before the upload starts.
    drop(coordinator);
    let coordinator = ArchiveCoordinator::new(store.clone());

    let stored = coordinator.staging_marker(KEY).unwrap().unwrap();
    assert_eq!(stored.state, StagingState::Intent);
    let resumed = coordinator.begin_staging(KEY).unwrap();
    assert_eq!(resumed.state, StagingState::Intent);
    assert_eq!(store.list("_archive/stage/").unwrap().len(), 1);

    coordinator
        .begin()
        .put(ArchiveObject::new(
            KEY,
            b"segment".to_vec(),
            "application/vnd.apache.parquet",
        ))
        .unwrap();
    let moved = coordinator
        .advance_staging(KEY, StagingState::Uploaded)
        .unwrap();
    assert_eq!(moved.state, StagingState::Uploaded);

    let later = coordinator.begin_staging(KEY).unwrap();
    assert_eq!(later.state, StagingState::Uploaded);
}

#[test]
fn committed_to_gc_pending_is_refused_and_the_marker_stays() {
    let (_dir, _store, coordinator) = setup();
    coordinator.begin_staging(KEY).unwrap();
    coordinator
        .advance_staging(KEY, StagingState::Uploaded)
        .unwrap();
    coordinator
        .advance_staging(KEY, StagingState::Committed)
        .unwrap();

    assert!(matches!(
        coordinator.advance_staging(KEY, StagingState::GcPending),
        Err(SegmentError::IllegalStagingTransition { .. })
    ));
    let stored = coordinator.staging_marker(KEY).unwrap().unwrap();
    assert_eq!(stored.state, StagingState::Committed);
}

#[test]
fn intent_and_uploaded_may_become_gc_pending() {
    let (_dir, _store, coordinator) = setup();
    let first = "archive/a.parquet";
    let second = "archive/b.parquet";

    coordinator.begin_staging(first).unwrap();
    let gone = coordinator
        .advance_staging(first, StagingState::GcPending)
        .unwrap();
    assert_eq!(gone.state, StagingState::GcPending);

    coordinator.begin_staging(second).unwrap();
    coordinator
        .advance_staging(second, StagingState::Uploaded)
        .unwrap();
    let gone = coordinator
        .advance_staging(second, StagingState::GcPending)
        .unwrap();
    assert_eq!(gone.state, StagingState::GcPending);

    assert!(matches!(
        coordinator.advance_staging(first, StagingState::Uploaded),
        Err(SegmentError::IllegalStagingTransition {
            from: StagingState::GcPending,
            to: StagingState::Uploaded,
        })
    ));
}

#[test]
fn advance_without_a_marker_is_an_error() {
    let (_dir, _store, coordinator) = setup();

    assert!(matches!(
        coordinator.advance_staging("archive/none.parquet", StagingState::Uploaded),
        Err(SegmentError::StagingMarkerMissing { .. })
    ));
    assert!(coordinator
        .staging_marker("archive/none.parquet")
        .unwrap()
        .is_none());
}

#[test]
fn a_corrupt_marker_is_an_error() {
    let (_dir, store, coordinator) = setup();
    store
        .put(MARKER_KEY, b"{", "application/json", PutCondition::IfAbsent)
        .unwrap();

    assert!(matches!(
        coordinator.staging_marker(KEY),
        Err(SegmentError::CorruptStagingMarker { .. })
    ));
}

#[test]
fn a_marker_round_trips_through_the_coordinator() {
    let (_dir, _store, coordinator) = setup();
    let marker = coordinator.begin_staging(KEY).unwrap();

    let stored: StagingMarker = coordinator.staging_marker(KEY).unwrap().unwrap();

    assert_eq!(stored, marker);
}
