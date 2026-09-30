use super::*;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn state(entries: Vec<(Index, Term, Vec<u8>)>) -> PersistedState {
    PersistedState {
        term: entries.last().map(|(_, term, _)| *term).unwrap_or(0),
        voted_for: None,
        commit_index: entries.last().map(|(index, _, _)| *index).unwrap_or(0),
        snapshot_index: 0,
        snapshot_term: 0,
        snapshot: Vec::new(),
        conf: None,
        log: entries
            .into_iter()
            .map(|(index, term, command)| RaftEntry {
                index,
                term,
                command,
                kind: EntryKind::Command,
            })
            .collect(),
    }
}

fn store(dir: &tempfile::TempDir) -> RaftStore {
    RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Always,
    )
    .unwrap()
}

#[test]
fn command_pin_maps_published_frame_after_append() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"first".to_vec())])).unwrap();
    let lease = store.pin_committed_command(1, 1).unwrap().map().unwrap();
    store
        .save(&state(vec![
            (1, 1, b"first".to_vec()),
            (2, 1, b"second".to_vec()),
        ]))
        .unwrap();

    assert_eq!(lease.command(), b"first");
    assert_eq!(
        store
            .pin_committed_command(2, 1)
            .unwrap()
            .map()
            .unwrap()
            .command(),
        b"second"
    );
}

#[test]
fn command_pin_survives_rewrite_and_superseded_generation_collection() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"old".to_vec())])).unwrap();
    let old_generation = store
        .cache
        .lock()
        .unwrap()
        .log
        .as_ref()
        .unwrap()
        .layout
        .generation;
    let pin = store.pin_committed_command(1, 1).unwrap();

    store.save(&state(vec![(2, 2, b"new".to_vec())])).unwrap();

    let lease = pin.map().unwrap();
    assert_eq!(lease.command(), b"old");
    assert!(store.log_artifact_path(&old_generation).exists());
    drop(lease);
    store.save(&state(vec![(3, 3, b"newer".to_vec())])).unwrap();
    assert!(!store.log_artifact_path(&old_generation).exists());
}

#[test]
fn reopened_store_rebuilds_published_command_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let first = store(&dir);
    first
        .save(&state(vec![(1, 7, b"reopen".to_vec())]))
        .unwrap();
    drop(first);

    let reopened = store(&dir);
    reopened.load().unwrap();
    let lease = reopened.pin_committed_command(1, 7).unwrap().map().unwrap();
    assert_eq!(lease.command(), b"reopen");
}

#[test]
fn command_pin_refuses_wrong_identity_and_unpublished_state() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store
        .save(&state(vec![(1, 3, b"published".to_vec())]))
        .unwrap();
    let published = store.pin_committed_command(1, 3).unwrap();
    store.inject_next_save_failure_with_kind(io::ErrorKind::Other);
    assert!(store
        .save(&state(vec![(2, 4, b"unpublished".to_vec())]))
        .is_err());

    assert_eq!(published.map().unwrap().command(), b"published");

    let wrong_term = match store.pin_committed_command(1, 4) {
        Ok(_) => panic!("wrong term must not pin a published command"),
        Err(error) => error,
    };
    assert_eq!(wrong_term.kind(), io::ErrorKind::InvalidInput);
    let unpublished = match store.pin_committed_command(2, 4) {
        Ok(_) => panic!("unpublished command must not pin"),
        Err(error) => error,
    };
    assert_eq!(unpublished.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn command_pin_refuses_published_but_uncommitted_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let mut durable = state(vec![(1, 3, b"uncommitted".to_vec())]);
    durable.commit_index = 0;
    store.save(&durable).unwrap();

    let error = match store.pin_committed_command(1, 3) {
        Ok(_) => panic!("uncommitted suffix must not pin"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn load_cannot_truncate_concurrently_appended_published_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(store(&dir));
    store.save(&state(vec![(1, 1, b"first".to_vec())])).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    store.pause_next_load_after_state_read(entered_tx, release_rx);
    let loading = {
        let store = Arc::clone(&store);
        thread::spawn(move || store.load())
    };
    let entered = entered_rx.recv_timeout(Duration::from_secs(1));
    if entered.is_err() {
        drop(release_tx);
        let _ = loading.join();
        panic!("load did not reach the post-read pause");
    }

    let (saved_tx, saved_rx) = mpsc::channel();
    let saving = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let result = store.save(&state(vec![
                (1, 1, b"first".to_vec()),
                (2, 1, b"second".to_vec()),
            ]));
            let _ = saved_tx.send(result);
        })
    };

    // A correct I/O mutex may keep this pending. Always release before join.
    let saved_before_release = saved_rx.recv_timeout(Duration::from_secs(1)).ok();
    let _ = release_tx.send(());
    let loaded = loading.join();
    let saving_join = saving.join();
    let saved = saved_before_release.or_else(|| saved_rx.recv_timeout(Duration::from_secs(1)).ok());
    assert!(loaded.is_ok(), "load worker panicked");
    assert!(saving_join.is_ok(), "save worker panicked");
    assert_eq!(loaded.unwrap().unwrap().unwrap().log[0].command, b"first");
    saved.expect("save did not finish after release").unwrap();

    let reopened = RaftStore::open(
        dir.path().to_str().unwrap(),
        NodeId::new(0),
        FsyncPolicy::Always,
    )
    .unwrap();
    assert_eq!(reopened.load().unwrap().unwrap().log[1].command, b"second");
    assert_eq!(
        reopened
            .pin_committed_command(2, 1)
            .unwrap()
            .map()
            .unwrap()
            .command(),
        b"second"
    );
}

#[test]
fn duplicate_append_identity_refuses_before_hard_state_publication() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"first".to_vec())])).unwrap();
    let hard_before = std::fs::read(store.path()).unwrap();
    let pin = store.pin_committed_command(1, 1).unwrap();

    let error = store
        .save(&state(vec![
            (1, 1, b"first".to_vec()),
            (1, 1, b"repeated".to_vec()),
        ]))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(std::fs::read(store.path()).unwrap(), hard_before);
    assert_eq!(pin.map().unwrap().command(), b"first");
}

#[test]
fn command_pin_map_refuses_missing_published_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"frame".to_vec())])).unwrap();
    let pin = store.pin_committed_command(1, 1).unwrap();
    let generation = store
        .cache
        .lock()
        .unwrap()
        .log
        .as_ref()
        .unwrap()
        .layout
        .generation;
    std::fs::remove_file(store.log_artifact_path(&generation)).unwrap();

    let error = match pin.map() {
        Ok(_) => panic!("missing artifact must not map"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn command_pin_map_refuses_crc_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"frame".to_vec())])).unwrap();
    let pin = store.pin_committed_command(1, 1).unwrap();
    let generation = store
        .cache
        .lock()
        .unwrap()
        .log
        .as_ref()
        .unwrap()
        .layout
        .generation;
    let path = store.log_artifact_path(&generation);
    let mut artifact = OpenOptions::new().write(true).open(path).unwrap();
    artifact.seek(SeekFrom::Start(8 + 12 + 25)).unwrap();
    artifact.write_all(b"X").unwrap();
    artifact.flush().unwrap();

    let error = match pin.map() {
        Ok(_) => panic!("corrupt artifact must not map"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn command_pin_map_refuses_truncated_published_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store.save(&state(vec![(1, 1, b"frame".to_vec())])).unwrap();
    let pin = store.pin_committed_command(1, 1).unwrap();
    let generation = store
        .cache
        .lock()
        .unwrap()
        .log
        .as_ref()
        .unwrap()
        .layout
        .generation;
    let artifact = OpenOptions::new()
        .write(true)
        .open(store.log_artifact_path(&generation))
        .unwrap();
    artifact.set_len(8).unwrap();

    let error = match pin.map() {
        Ok(_) => panic!("truncated artifact must not map"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
