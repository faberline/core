use super::*;

#[test]
fn synchronous_mapped_trim_retains_readable_appender_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synchronous.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();
    log.truncate_mapped_in_place(1).unwrap();
    assert_eq!(replay_sequences(&path), vec![2]);

    let mut retained = log.file.get_ref().try_clone().unwrap();
    retained.seek(SeekFrom::Start(0)).unwrap();
    let mut header = [0u8; HEADER_LEN];
    retained.read_exact(&mut header).unwrap();
    assert_eq!(u64::from_le_bytes(header[..8].try_into().unwrap()), 2);
    log.append(3, b"after publication").unwrap();
    drop(retained);
    drop(log);
    assert_eq!(replay_sequences(&path), vec![2, 3]);

    let mut reopened = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    reopened.append(4, b"after reopen").unwrap();
    drop(reopened);
    assert_eq!(replay_sequences(&path), vec![2, 3, 4]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_ready_prefix_and_exact_suffix_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    for sequence in 1..=3 {
        log.append(sequence, &sequence.to_le_bytes()).unwrap();
    }

    let mut plan = log.begin_trim_mapped(1).unwrap();
    plan.copy_stable_prefix().unwrap();
    log.append(4, b"late suffix").unwrap();
    log.finish_trim_mapped(plan).unwrap();
    drop(log);

    assert_eq!(replay_sequences(&path), vec![2, 3, 4]);
    let mut reopened = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    reopened.append(5, b"after reopen").unwrap();
    drop(reopened);
    assert_eq!(replay_sequences(&path), vec![2, 3, 4, 5]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_requires_ready_full_prefix_before_publication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();

    let plan = log.begin_trim_mapped(1).unwrap();
    assert!(log.finish_trim_mapped(plan).is_err());
    drop(log);
    assert_eq!(replay_sequences(&path), vec![1, 2]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_busy_preserves_active_fixed_temp_and_drop_releases_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();

    let plan = log.begin_trim_mapped(1).unwrap();
    let tmp = log.compact_tmp_path();
    let mut marker = OpenOptions::new().append(true).open(&tmp).unwrap();
    marker.write_all(b"active-plan-owned-temp").unwrap();
    marker.sync_all().unwrap();
    let before = std::fs::read(&tmp).unwrap();
    assert!(log
        .begin_trim_mapped(1)
        .unwrap_err()
        .to_string()
        .contains("Busy"));
    assert!(log
        .truncate_through(1)
        .unwrap_err()
        .to_string()
        .contains("Busy"));
    assert_eq!(std::fs::read(&tmp).unwrap(), before);

    drop(plan);
    log.truncate_through(1).unwrap();
    log.append(3, b"after generic trim").unwrap();
    let mut replacement = log.begin_trim_mapped(1).unwrap();
    replacement.copy_stable_prefix().unwrap();
    log.finish_trim_mapped(replacement).unwrap();
    assert_eq!(replay_sequences(&path), vec![2, 3]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_rejects_wrong_writer_revision_and_replaced_inode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();

    let mut wrong_writer_plan = log.begin_trim_mapped(1).unwrap();
    wrong_writer_plan.copy_stable_prefix().unwrap();
    let mut other = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    assert!(other.finish_trim_mapped(wrong_writer_plan).is_err());
    assert_eq!(replay_sequences(&path), vec![1, 2]);

    let mut revision_plan = log.begin_trim_mapped(1).unwrap();
    revision_plan.copy_stable_prefix().unwrap();
    log.trim_revision += 1;
    assert!(log.finish_trim_mapped(revision_plan).is_err());
    assert_eq!(replay_sequences(&path), vec![1, 2]);

    let mut inode_plan = log.begin_trim_mapped(1).unwrap();
    inode_plan.copy_stable_prefix().unwrap();
    let replacement = dir.path().join("replacement.log");
    std::fs::write(&replacement, []).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let replacement_bytes = std::fs::read(&path).unwrap();
    assert!(log.finish_trim_mapped(inode_plan).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), replacement_bytes);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_streams_large_payload_in_multiple_fixed_chunks() {
    const CHUNK: usize = 64 * 1024;
    const PAYLOAD_BYTES: usize = CHUNK * 3 + 17;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.log");
    let payload = vec![0x5a; PAYLOAD_BYTES];
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append_large_payload(2, &payload).unwrap();

    let mut plan = log.begin_trim_mapped(1).unwrap();
    let observation = AllocationObservation::start();
    plan.copy_stable_prefix().unwrap();
    let largest_request = observation.largest_request();
    drop(observation);
    log.finish_trim_mapped(plan).unwrap();

    assert!(largest_request <= CHUNK);
    assert_eq!(replay_sequences(&path), vec![2]);
}
