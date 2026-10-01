use super::*;

#[test]
fn large_mapped_api_retains_a_valid_legacy_frame_without_large_read_allocation() {
    const MAX_ALLOWED_REQUEST: usize = 64 * 1024;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let payload = vec![0x5a; LARGE_PAYLOAD_BYTES];
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    assert!(log.append(2, &payload).is_err());
    log.append(1, b"discarded prefix").unwrap();
    log.append_large_payload(2, &payload).unwrap();
    log.sync().unwrap();
    let mut pinned_cursor = FramedLogCursor::open(&path).unwrap();

    let mut default_cursor = FramedLogCursor::open(&path).unwrap();
    assert_eq!(default_cursor.next_mapped_frame().unwrap().unwrap().seq, 1);
    assert!(default_cursor.next_mapped_frame().is_err());

    let mut cursor = FramedLogCursor::open(&path).unwrap();
    assert_eq!(cursor.next_large_mapped_frame().unwrap().unwrap().seq, 1);
    let large_offset = cursor.byte_offset();
    let observation = AllocationObservation::start();
    let large = cursor.next_large_mapped_frame().unwrap().unwrap();
    let largest_request = observation.largest_request();
    assert_eq!(large.seq, 2);
    assert_eq!(large.payload(), payload.as_slice());
    assert!(largest_request <= MAX_ALLOWED_REQUEST);
    drop(large);
    drop(observation);

    log.truncate_through_mapped(1).unwrap();
    log.append(3, b"post-compaction append").unwrap();
    log.sync().unwrap();
    let reread = cursor
        .reread_large_mapped_frame_at(large_offset)
        .unwrap()
        .unwrap();
    assert_eq!(reread.seq, 2);
    assert_eq!(reread.payload(), payload.as_slice());
    drop(reread);

    let mut compacted = FramedLogCursor::open(&path).unwrap();
    let retained = compacted.next_large_mapped_frame().unwrap().unwrap();
    assert_eq!(retained.seq, 2);
    assert_eq!(retained.payload(), payload.as_slice());
    let appended = compacted.next_large_mapped_frame().unwrap().unwrap();
    assert_eq!(appended.seq, 3);
    assert_eq!(appended.payload(), b"post-compaction append");
    assert!(compacted.next_large_mapped_frame().unwrap().is_none());

    assert_eq!(
        pinned_cursor
            .next_large_mapped_frame()
            .unwrap()
            .unwrap()
            .seq,
        1
    );
    assert_eq!(
        pinned_cursor
            .next_large_mapped_frame()
            .unwrap()
            .unwrap()
            .seq,
        2
    );
    assert!(pinned_cursor.next_large_mapped_frame().unwrap().is_none());
}

#[test]
fn large_mapped_api_reports_incomplete_and_bad_crc_without_changing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let incomplete_path = dir.path().join("incomplete.log");
    let mut incomplete = File::create(&incomplete_path).unwrap();
    incomplete.write_all(&9u64.to_le_bytes()).unwrap();
    incomplete
        .write_all(&(LARGE_PAYLOAD_BYTES as u32).to_le_bytes())
        .unwrap();
    incomplete.write_all(&0u32.to_le_bytes()).unwrap();
    incomplete.sync_all().unwrap();
    let before_incomplete = std::fs::metadata(&incomplete_path).unwrap().len();
    let mut incomplete_cursor = FramedLogCursor::open(&incomplete_path).unwrap();
    let error = incomplete_cursor.next_large_mapped_frame().unwrap_err();
    assert!(error.to_string().contains("oversized legacy log frame"));
    assert_eq!(
        std::fs::metadata(&incomplete_path).unwrap().len(),
        before_incomplete
    );

    let bad_crc_path = dir.path().join("bad-crc.log");
    let mut bad_crc = File::create(&bad_crc_path).unwrap();
    bad_crc.write_all(&10u64.to_le_bytes()).unwrap();
    bad_crc
        .write_all(&(LARGE_PAYLOAD_BYTES as u32).to_le_bytes())
        .unwrap();
    bad_crc.write_all(&1u32.to_le_bytes()).unwrap();
    bad_crc
        .set_len(HEADER_LEN as u64 + LARGE_PAYLOAD_BYTES as u64)
        .unwrap();
    bad_crc.sync_all().unwrap();
    let before_bad_crc = std::fs::metadata(&bad_crc_path).unwrap().len();
    let mut bad_crc_cursor = FramedLogCursor::open(&bad_crc_path).unwrap();
    let error = bad_crc_cursor.next_large_mapped_frame().unwrap_err();
    assert!(error.to_string().contains("failed validation"));
    assert_eq!(
        std::fs::metadata(&bad_crc_path).unwrap().len(),
        before_bad_crc
    );
}

#[test]
fn mapped_truncation_does_not_publish_when_a_large_frame_fails_validation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(1, b"original prefix").unwrap();
    log.sync().unwrap();

    let mut corrupt = OpenOptions::new().append(true).open(&path).unwrap();
    corrupt.write_all(&2u64.to_le_bytes()).unwrap();
    corrupt
        .write_all(&(LARGE_PAYLOAD_BYTES as u32).to_le_bytes())
        .unwrap();
    corrupt.write_all(&1u32.to_le_bytes()).unwrap();
    corrupt
        .set_len(std::fs::metadata(&path).unwrap().len() + LARGE_PAYLOAD_BYTES as u64)
        .unwrap();
    corrupt.sync_all().unwrap();
    let before = std::fs::metadata(&path).unwrap().len();

    let error = log.truncate_through_mapped(0).unwrap_err();
    assert!(error.to_string().contains("failed validation"));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), before);
    let mut cursor = FramedLogCursor::open(&path).unwrap();
    assert_eq!(cursor.next_large_mapped_frame().unwrap().unwrap().seq, 1);
    assert!(cursor.next_large_mapped_frame().is_err());
}
