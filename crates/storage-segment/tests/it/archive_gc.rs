use std::sync::Arc;

use storage_object::{LocalObjectStore, ObjectStore};
use storage_segment::{ArchiveCoordinator, ArchiveObject, SegmentError, StagingState};

const RETENTION: u64 = 100;
const CONTENT_TYPE: &str = "application/vnd.apache.parquet";
const STAGE_PREFIX: &str = "_archive/stage/";

fn setup() -> (tempfile::TempDir, Arc<LocalObjectStore>, ArchiveCoordinator) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let coordinator = ArchiveCoordinator::new(store.clone());
    (dir, store, coordinator)
}

fn put_segment(coordinator: &ArchiveCoordinator, key: &str) {
    coordinator
        .begin()
        .put(ArchiveObject::new(key, b"segment".to_vec(), CONTENT_TYPE))
        .unwrap();
}

fn exists(store: &LocalObjectStore, key: &str) -> bool {
    store.head(key).is_ok()
}

fn stage_gc_pending(coordinator: &ArchiveCoordinator, key: &str, at: u64) {
    coordinator.begin_staging(key).unwrap();
    put_segment(coordinator, key);
    coordinator
        .advance_staging_at(key, StagingState::GcPending, at)
        .unwrap();
}

fn stage_committed(coordinator: &ArchiveCoordinator, key: &str, at: u64) {
    coordinator.begin_staging(key).unwrap();
    put_segment(coordinator, key);
    coordinator
        .advance_staging(key, StagingState::Uploaded)
        .unwrap();
    coordinator
        .advance_staging_at(key, StagingState::Committed, at)
        .unwrap();
}

#[test]
fn gc_deletes_a_gc_pending_object_past_the_fence() {
    let (_dir, store, coordinator) = setup();
    stage_gc_pending(&coordinator, "archive/a.parquet", 1000);

    let report = coordinator.collect_garbage(1100, RETENTION).unwrap();

    assert_eq!(report.deleted, vec!["archive/a.parquet".to_string()]);
    assert!(!exists(&store, "archive/a.parquet"));
    assert!(store.list(STAGE_PREFIX).unwrap().is_empty());
}

#[test]
fn gc_keeps_a_gc_pending_object_inside_the_window() {
    let (_dir, store, coordinator) = setup();
    stage_gc_pending(&coordinator, "archive/a.parquet", 1000);

    let report = coordinator.collect_garbage(1099, RETENTION).unwrap();

    assert!(report.deleted.is_empty());
    assert!(exists(&store, "archive/a.parquet"));
    assert_eq!(store.list(STAGE_PREFIX).unwrap().len(), 1);
}

#[test]
fn gc_never_deletes_a_segment_that_is_not_staged() {
    let (_dir, store, coordinator) = setup();
    put_segment(&coordinator, "archive/plain.parquet");

    let report = coordinator.collect_garbage(u64::MAX, 0).unwrap();

    assert!(report.deleted.is_empty());
    assert!(exists(&store, "archive/plain.parquet"));
}

#[test]
fn gc_never_deletes_committed_intent_or_uploaded_objects() {
    let (_dir, store, coordinator) = setup();
    stage_committed(&coordinator, "archive/c.parquet", 10);
    coordinator.begin_staging("archive/i.parquet").unwrap();
    put_segment(&coordinator, "archive/i.parquet");
    coordinator.begin_staging("archive/u.parquet").unwrap();
    put_segment(&coordinator, "archive/u.parquet");
    coordinator
        .advance_staging("archive/u.parquet", StagingState::Uploaded)
        .unwrap();

    let report = coordinator.collect_garbage(u64::MAX, 0).unwrap();

    assert!(report.deleted.is_empty());
    for key in [
        "archive/c.parquet",
        "archive/i.parquet",
        "archive/u.parquet",
    ] {
        assert!(exists(&store, key), "{key} must stay");
    }
    assert_eq!(store.list(STAGE_PREFIX).unwrap().len(), 3);
}

#[test]
fn gc_is_idempotent() {
    let (_dir, store, coordinator) = setup();
    stage_gc_pending(&coordinator, "archive/a.parquet", 1000);
    stage_gc_pending(&coordinator, "archive/b.parquet", 1000);

    let first = coordinator.collect_garbage(2000, RETENTION).unwrap();
    let mut deleted = first.deleted.clone();
    deleted.sort();
    assert_eq!(
        deleted,
        vec![
            "archive/a.parquet".to_string(),
            "archive/b.parquet".to_string()
        ]
    );
    assert!(!exists(&store, "archive/a.parquet"));
    assert!(!exists(&store, "archive/b.parquet"));

    let second = coordinator.collect_garbage(2000, RETENTION).unwrap();
    assert!(second.deleted.is_empty());
}

#[test]
fn gc_repairs_a_crash_between_the_segment_delete_and_the_marker_delete() {
    let (_dir, store, coordinator) = setup();
    stage_gc_pending(&coordinator, "archive/a.parquet", 1000);
    // Simulate the crash: the segment is gone, the marker is not.
    store.delete("archive/a.parquet").unwrap();

    let report = coordinator.collect_garbage(2000, RETENTION).unwrap();

    assert!(report.deleted.is_empty(), "the segment was already gone");
    assert!(store.list(STAGE_PREFIX).unwrap().is_empty());
}

#[test]
fn delete_committed_inside_the_window_is_refused() {
    let (_dir, store, coordinator) = setup();
    stage_committed(&coordinator, "archive/c.parquet", 1000);

    let result = coordinator.delete_committed("archive/c.parquet", 1099, RETENTION);

    assert!(
        matches!(
            result,
            Err(SegmentError::InsideRetentionWindow { until: 1100, .. })
        ),
        "{result:?}"
    );
    assert!(exists(&store, "archive/c.parquet"));
    assert_eq!(store.list(STAGE_PREFIX).unwrap().len(), 1);
}

#[test]
fn delete_committed_after_the_window_deletes_segment_and_marker() {
    let (_dir, store, coordinator) = setup();
    stage_committed(&coordinator, "archive/c.parquet", 1000);

    let result = coordinator.delete_committed("archive/c.parquet", 1100, RETENTION);

    assert!(result.is_ok(), "{result:?}");
    assert!(!exists(&store, "archive/c.parquet"));
    assert!(store.list(STAGE_PREFIX).unwrap().is_empty());
}

#[test]
fn delete_committed_refuses_a_marker_without_a_time() {
    let (_dir, store, coordinator) = setup();
    coordinator.begin_staging("archive/c.parquet").unwrap();
    put_segment(&coordinator, "archive/c.parquet");
    coordinator
        .advance_staging("archive/c.parquet", StagingState::Uploaded)
        .unwrap();
    coordinator
        .advance_staging("archive/c.parquet", StagingState::Committed)
        .unwrap();

    let result = coordinator.delete_committed("archive/c.parquet", u64::MAX - 1, 0);

    assert!(
        matches!(result, Err(SegmentError::InsideRetentionWindow { .. })),
        "{result:?}"
    );
    assert!(exists(&store, "archive/c.parquet"));
}

#[test]
fn delete_committed_refuses_other_states_and_unstaged_segments() {
    let (_dir, store, coordinator) = setup();
    stage_gc_pending(&coordinator, "archive/g.parquet", 1000);
    put_segment(&coordinator, "archive/plain.parquet");

    let gc_pending = coordinator.delete_committed("archive/g.parquet", u64::MAX, 0);
    assert!(
        matches!(gc_pending, Err(SegmentError::SegmentNotCommitted { .. })),
        "{gc_pending:?}"
    );
    let unstaged = coordinator.delete_committed("archive/plain.parquet", u64::MAX, 0);
    assert!(
        matches!(unstaged, Err(SegmentError::StagingMarkerMissing { .. })),
        "{unstaged:?}"
    );
    assert!(exists(&store, "archive/g.parquet"));
    assert!(exists(&store, "archive/plain.parquet"));
}
