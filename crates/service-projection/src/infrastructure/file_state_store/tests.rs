use chrono::{TimeZone, Utc};

use super::FileProjectionStateStore;
use crate::domain::{checkpoint, ProjectionDescriptor, ProjectionStateStore};

const STATE: &[u8] = br#"{"count":1}"#;

fn descriptor(schema_version: u32) -> ProjectionDescriptor {
    ProjectionDescriptor {
        name: "logs".to_string(),
        schema_version,
        retention: "7d".to_string(),
    }
}

#[test]
fn persisted_state_reads_back_and_restores() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileProjectionStateStore::new(dir.path());
    store.prepare_root().unwrap();
    assert!(dir.path().join("indexes").is_dir());
    assert!(store.read("logs").unwrap().is_none());

    let now = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    let saved = checkpoint(
        &descriptor(2),
        42,
        3,
        Some("evt-42".to_string()),
        STATE,
        now,
    );
    store.persist("logs", &saved, STATE).unwrap();
    assert!(dir.path().join("indexes/logs/state.json").is_file());

    let bytes = store.read("logs").unwrap().unwrap();
    let (restored, state) = store.restore(&descriptor(2), &bytes).unwrap();
    assert_eq!(restored, saved);
    assert_eq!(state, STATE);
    assert!(store.restore(&descriptor(3), &bytes).is_err());
}

#[test]
fn quarantine_moves_the_state_file_aside() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileProjectionStateStore::new(dir.path());
    store.prepare_root().unwrap();
    let state_dir = dir.path().join("indexes/logs");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::write(state_dir.join("state.json"), b"{\"truncated\":").unwrap();

    let bytes = store.read("logs").unwrap().unwrap();
    assert!(store.restore(&descriptor(2), &bytes).is_err());
    store.quarantine("logs", &bytes).unwrap();

    assert!(store.read("logs").unwrap().is_none());
    let names: Vec<String> = std::fs::read_dir(&state_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].starts_with("state.corrupt-"));
}
