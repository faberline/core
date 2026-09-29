use super::*;

#[test]
#[cfg(unix)]
fn mapped_trim_plan_invalid_prefix_or_suffix_never_replaces_old_aof() {
    let dir = tempfile::tempdir().unwrap();
    let prefix_path = dir.path().join("prefix.log");
    let mut prefix = FramedLogWriter::open(&prefix_path, FsyncPolicy::Always).unwrap();
    prefix.append(1, b"covered").unwrap();
    prefix.append(2, b"retained").unwrap();
    prefix.sync().unwrap();
    let mut corrupt = OpenOptions::new().write(true).open(&prefix_path).unwrap();
    corrupt.seek(SeekFrom::Start(HEADER_LEN as u64)).unwrap();
    corrupt.write_all(b"!").unwrap();
    corrupt.sync_all().unwrap();
    let before_prefix = std::fs::metadata(&prefix_path).unwrap().len();
    let mut prefix_plan = prefix.begin_trim_mapped(1).unwrap();
    assert!(prefix_plan.copy_stable_prefix().is_err());
    assert!(prefix_plan.copy_stable_prefix().is_err());
    assert!(prefix.finish_trim_mapped(prefix_plan).is_err());
    assert_eq!(
        std::fs::metadata(&prefix_path).unwrap().len(),
        before_prefix
    );

    let torn_prefix_path = dir.path().join("torn-prefix.log");
    let mut torn_prefix = FramedLogWriter::open(&torn_prefix_path, FsyncPolicy::Always).unwrap();
    torn_prefix.append(1, b"covered").unwrap();
    torn_prefix.append(2, b"retained").unwrap();
    torn_prefix.sync().unwrap();
    let mut torn_prefix_bytes = OpenOptions::new()
        .append(true)
        .open(&torn_prefix_path)
        .unwrap();
    torn_prefix_bytes.write_all(&3u64.to_le_bytes()).unwrap();
    torn_prefix_bytes.sync_all().unwrap();
    let mut torn_prefix_plan = torn_prefix.begin_trim_mapped(3).unwrap();
    assert!(torn_prefix_plan.copy_stable_prefix().is_err());
    assert!(torn_prefix.finish_trim_mapped(torn_prefix_plan).is_err());
    drop(torn_prefix);
    assert_eq!(replay_sequences(&torn_prefix_path), vec![1, 2]);

    let suffix_path = dir.path().join("suffix.log");
    let mut suffix = FramedLogWriter::open(&suffix_path, FsyncPolicy::Always).unwrap();
    suffix.append(1, b"covered").unwrap();
    suffix.append(2, b"retained").unwrap();
    let mut suffix_plan = suffix.begin_trim_mapped(1).unwrap();
    suffix_plan.copy_stable_prefix().unwrap();
    let mut corrupt_suffix = OpenOptions::new().append(true).open(&suffix_path).unwrap();
    corrupt_suffix.write_all(&3u64.to_le_bytes()).unwrap();
    corrupt_suffix.write_all(&4u32.to_le_bytes()).unwrap();
    corrupt_suffix.write_all(&0u32.to_le_bytes()).unwrap();
    corrupt_suffix.write_all(b"crc!").unwrap();
    corrupt_suffix.sync_all().unwrap();
    let before_suffix = std::fs::metadata(&suffix_path).unwrap().len();
    assert!(suffix.finish_trim_mapped(suffix_plan).is_err());
    assert_eq!(
        std::fs::metadata(&suffix_path).unwrap().len(),
        before_suffix
    );
    drop(suffix);
    assert_eq!(replay_sequences(&suffix_path), vec![1, 2]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_torn_suffix_keeps_exact_original_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("torn-suffix.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();
    let mut plan = log.begin_trim_mapped(1).unwrap();
    plan.copy_stable_prefix().unwrap();
    log.append(3, b"acknowledged suffix").unwrap();
    let mut tail = OpenOptions::new().append(true).open(&path).unwrap();
    tail.write_all(&4u64.to_le_bytes()).unwrap();
    tail.sync_all().unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(log.finish_trim_mapped(plan).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(replay_sequences(&path), vec![1, 2, 3]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_failed_begin_and_abandoned_temp_do_not_strand_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("begin.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    let tmp = log.compact_tmp_path();
    std::fs::create_dir(&tmp).unwrap();
    assert!(log.begin_trim_mapped(1).is_err());
    assert!(log.trim_active.lock().unwrap().is_none());
    std::fs::remove_dir(&tmp).unwrap();
    std::fs::write(&tmp, b"abandoned prior attempt").unwrap();
    let mut plan = log.begin_trim_mapped(1).unwrap();
    assert!(std::fs::read(&tmp).unwrap().is_empty());
    plan.copy_stable_prefix().unwrap();
    log.finish_trim_mapped(plan).unwrap();
    assert!(!tmp.exists());
    assert!(log.trim_active.lock().unwrap().is_none());
    assert!(replay_sequences(&path).is_empty());
}

#[test]
#[cfg(unix)]
fn mapped_trim_rejects_changed_temp_and_repeated_ready_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temp-identity.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut plan = log.begin_trim_mapped(1).unwrap();
    plan.copy_stable_prefix().unwrap();
    assert!(plan.copy_stable_prefix().is_err());
    assert!(log.finish_trim_mapped(plan).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut plan = log.begin_trim_mapped(1).unwrap();
    plan.copy_stable_prefix().unwrap();
    let replacement = dir.path().join("replacement.tmp");
    std::fs::write(&replacement, b"unrelated inode").unwrap();
    std::fs::rename(&replacement, log.compact_tmp_path()).unwrap();
    assert!(log.finish_trim_mapped(plan).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    log.append(3, b"after rejected temp").unwrap();
    assert_eq!(replay_sequences(&path), vec![1, 2, 3]);
}

#[test]
#[cfg(unix)]
fn mapped_trim_plan_faults_preserve_pre_and_post_rename_contracts() {
    for point in [
        TrimFaultPoint::PrefixTempSync,
        TrimFaultPoint::SuffixTempSync,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.log");
        let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
        log.append(1, b"covered").unwrap();
        log.append(2, b"retained").unwrap();
        let faults = log.trim_faults_for_test();
        faults.fail_once(point);
        let mut plan = log.begin_trim_mapped(1).unwrap();
        let result = plan.copy_stable_prefix();
        if point == TrimFaultPoint::SuffixTempSync {
            result.unwrap();
            log.append(3, b"suffix").unwrap();
            assert!(log.finish_trim_mapped(plan).is_err());
        } else {
            assert!(result.is_err());
            drop(plan);
            assert!(log.trim_active.lock().unwrap().is_none());
            assert!(!log.compact_tmp_path().exists());
            let retry = log.begin_trim_mapped(1).unwrap();
            drop(retry);
        }
        drop(log);
        let expected = if point == TrimFaultPoint::SuffixTempSync {
            vec![1, 2, 3]
        } else {
            vec![1, 2]
        };
        assert_eq!(replay_sequences(&path), expected);
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parent-sync.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"covered").unwrap();
    log.append(2, b"retained").unwrap();
    let faults = log.trim_faults_for_test();
    faults.fail_once(TrimFaultPoint::ParentSyncAfterRename);
    let mut plan = log.begin_trim_mapped(1).unwrap();
    plan.copy_stable_prefix().unwrap();
    assert!(log.finish_trim_mapped(plan).is_err());
    log.append(3, b"later append").unwrap();
    drop(log);
    assert_eq!(replay_sequences(&path), vec![2, 3]);
}
