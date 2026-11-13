use std::sync::Arc;

use storage_object::{LocalObjectStore, ObjectStore, PutCondition};
use storage_segment::{ArchiveCoordinator, ArchiveObject, SegmentError, StagingState};

const SEGMENT_KEY: &str = "archive/segment-0001.parquet";
const MARKER_KEY: &str =
    "_archive/stage/fb6c3989579d2eae9aff55014178ae1764a8ef8b1ad93c0294d46d8ce7fac5b2.json";
const CONTENT_TYPE: &str = "application/vnd.apache.parquet";

fn setup() -> (tempfile::TempDir, Arc<LocalObjectStore>, ArchiveCoordinator) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let coordinator = ArchiveCoordinator::new(store.clone());
    (dir, store, coordinator)
}

#[test]
fn begin_staging_writes_an_intent_marker_the_store_lists() {
    let (_dir, store, coordinator) = setup();

    let marker = coordinator.begin_staging(SEGMENT_KEY).unwrap();

    assert_eq!(marker.state, StagingState::Intent);
    let listed = store.list("_archive/stage/").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key(), MARKER_KEY);
}

#[test]
fn marker_bytes_are_pinned() {
    let (dir, _store, coordinator) = setup();
    let path = dir.path().join(MARKER_KEY);

    coordinator.begin_staging(SEGMENT_KEY).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"intent"}"#
    );

    coordinator
        .advance_staging(SEGMENT_KEY, StagingState::Uploaded)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"uploaded"}"#
    );
}

#[test]
fn full_path_intent_uploaded_committed() {
    let (_dir, _store, coordinator) = setup();

    coordinator.begin_staging(SEGMENT_KEY).unwrap();
    assert_eq!(
        coordinator
            .advance_staging(SEGMENT_KEY, StagingState::Uploaded)
            .unwrap()
            .state,
        StagingState::Uploaded
    );
    assert_eq!(
        coordinator
            .advance_staging(SEGMENT_KEY, StagingState::Committed)
            .unwrap()
            .state,
        StagingState::Committed
    );
    assert_eq!(
        coordinator
            .staging_marker(SEGMENT_KEY)
            .unwrap()
            .unwrap()
            .state,
        StagingState::Committed
    );
}

#[test]
fn crash_between_intent_and_upload_resumes_without_a_second_intent() {
    let (_dir, store, coordinator) = setup();
    coordinator.begin_staging(SEGMENT_KEY).unwrap();

    // The crash: the coordinator is dropped before the upload starts.
    drop(coordinator);
    let coordinator = ArchiveCoordinator::new(store.clone());

    assert_eq!(
        coordinator
            .staging_marker(SEGMENT_KEY)
            .unwrap()
            .unwrap()
            .state,
        StagingState::Intent
    );
    assert_eq!(
        coordinator.begin_staging(SEGMENT_KEY).unwrap().state,
        StagingState::Intent
    );
    assert_eq!(store.list("_archive/stage/").unwrap().len(), 1);

    let mut transaction = coordinator.begin();
    transaction
        .put(ArchiveObject::new(
            SEGMENT_KEY,
            b"segment".to_vec(),
            CONTENT_TYPE,
        ))
        .unwrap();
    assert_eq!(
        coordinator
            .advance_staging(SEGMENT_KEY, StagingState::Uploaded)
            .unwrap()
            .state,
        StagingState::Uploaded
    );
    assert_eq!(
        coordinator.begin_staging(SEGMENT_KEY).unwrap().state,
        StagingState::Uploaded
    );
}

#[test]
fn committed_to_gc_pending_is_refused_and_the_marker_stays() {
    let (_dir, _store, coordinator) = setup();
    coordinator.begin_staging(SEGMENT_KEY).unwrap();
    coordinator
        .advance_staging(SEGMENT_KEY, StagingState::Uploaded)
        .unwrap();
    coordinator
        .advance_staging(SEGMENT_KEY, StagingState::Committed)
        .unwrap();

    assert!(matches!(
        coordinator.advance_staging(SEGMENT_KEY, StagingState::GcPending),
        Err(SegmentError::IllegalStagingTransition { .. })
    ));
    assert_eq!(
        coordinator
            .staging_marker(SEGMENT_KEY)
            .unwrap()
            .unwrap()
            .state,
        StagingState::Committed
    );
}

#[test]
fn intent_and_uploaded_may_become_gc_pending() {
    let (_dir, _store, coordinator) = setup();
    let first = "archive/a.parquet";
    let second = "archive/b.parquet";

    coordinator.begin_staging(first).unwrap();
    assert_eq!(
        coordinator
            .advance_staging(first, StagingState::GcPending)
            .unwrap()
            .state,
        StagingState::GcPending
    );

    coordinator.begin_staging(second).unwrap();
    coordinator
        .advance_staging(second, StagingState::Uploaded)
        .unwrap();
    assert_eq!(
        coordinator
            .advance_staging(second, StagingState::GcPending)
            .unwrap()
            .state,
        StagingState::GcPending
    );

    assert!(matches!(
        coordinator.advance_staging(first, StagingState::Uploaded),
        Err(SegmentError::IllegalStagingTransition {
            from: StagingState::GcPending,
            to: StagingState::Uploaded
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
        coordinator.staging_marker(SEGMENT_KEY),
        Err(SegmentError::CorruptStagingMarker { .. })
    ));
}
