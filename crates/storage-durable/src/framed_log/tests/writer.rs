use super::*;

#[test]
fn every_sec_sync_plan_keeps_newer_append_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aof.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::EverySec).unwrap();
    log.append(1, b"old").unwrap();
    log.last_sync = Instant::now() - Duration::from_secs(2);
    let plan = log.begin_sync().unwrap().expect("sync plan");
    log.append(2, b"new").unwrap();
    log.finish_sync(plan).unwrap();
    assert!(log.dirty, "a stale plan must not clear a newer append");

    log.last_sync = Instant::now() - Duration::from_secs(2);
    let next = log.begin_sync().unwrap().expect("replacement sync plan");
    log.finish_sync(next).unwrap();
    assert!(
        !log.dirty,
        "the replacement plan clears the current revision"
    );
}

#[test]
fn append_replay_truncate_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"one").unwrap();
    log.append(2, b"two").unwrap();
    log.append(3, b"three").unwrap();
    log.truncate_through(1).unwrap();
    log.append(4, b"four").unwrap();
    log.sync().unwrap();

    let frames = FramedLogReader::read_frames(&path, 0).unwrap();
    let seqs: Vec<u64> = frames.iter().map(|frame| frame.seq).collect();
    assert_eq!(seqs, vec![2, 3, 4]);
}

#[test]
fn torn_tail_replays_prefix_and_open_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"one").unwrap();
    log.append(2, b"two").unwrap();
    log.sync().unwrap();
    let good_len = std::fs::metadata(&path).unwrap().len();
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&99u64.to_le_bytes()).unwrap();
        file.sync_all().unwrap();
    }
    assert_eq!(FramedLogReader::read_frames(&path, 0).unwrap().len(), 2);
    let _ = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), good_len);
}

#[test]
fn strict_sync_persists_file_and_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"one").unwrap();
    log.sync_strict().unwrap();
    assert_eq!(FramedLogReader::read_frames(&path, 0).unwrap().len(), 1);
}

#[test]
fn bounded_reader_pages_without_returning_skipped_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    for sequence in 1..=5 {
        log.append(sequence, format!("frame-{sequence}").as_bytes())
            .unwrap();
    }
    log.sync().unwrap();

    let page = FramedLogReader::read_frames_bounded(&path, 2, 2).unwrap();
    assert_eq!(
        page.iter().map(|frame| frame.seq).collect::<Vec<_>>(),
        [3, 4]
    );
}
