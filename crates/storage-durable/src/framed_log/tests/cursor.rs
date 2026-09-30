use super::*;

#[test]
fn mapped_cursor_keeps_original_file_alive_after_path_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let replacement = dir.path().join("replacement.log");
    let payload = b"original mapped payload";
    let mut writer = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    writer.append(7, payload).unwrap();
    writer.sync().unwrap();
    let mut owned_cursor = FramedLogCursor::open(&path).unwrap();
    let owned = owned_cursor.next_frame().unwrap().unwrap();
    let mut cursor = FramedLogCursor::open(&path).unwrap();
    let view = cursor.next_mapped_frame().unwrap().unwrap();

    let mut replacement_writer = FramedLogWriter::open(&replacement, FsyncPolicy::Always).unwrap();
    replacement_writer
        .append(8, b"replacement payload")
        .unwrap();
    replacement_writer.sync().unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    drop(cursor);

    assert_eq!(view.seq, owned.seq());
    assert_eq!(view.payload(), owned.payload());
    assert_eq!(FramedLogReader::read_frames(&path, 0).unwrap()[0].seq(), 8);
    assert_eq!(view.payload(), payload);
}

#[test]
fn mapped_cursor_uses_open_length_and_reread_does_not_advance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.log");
    let mut writer = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    writer.append(1, b"prefix").unwrap();
    writer.sync().unwrap();
    let mut cursor = FramedLogCursor::open(&path).unwrap();
    let start = cursor.byte_offset();
    let view = cursor.next_mapped_frame().unwrap().unwrap();
    let next = cursor.byte_offset();
    writer.append(2, b"later append").unwrap();
    writer.sync().unwrap();

    assert_eq!(view.seq, 1);
    assert_eq!(view.payload(), b"prefix");
    fn requires_borrowed_payload(_: &[u8]) {}
    requires_borrowed_payload(view.payload());
    assert!(cursor.next_mapped_frame().unwrap().is_none());
    let reread = cursor.reread_large_mapped_frame_at(start).unwrap().unwrap();
    assert_eq!(reread.seq, 1);
    assert_eq!(reread.payload(), b"prefix");
    assert_eq!(cursor.byte_offset(), next);
    assert!(cursor.reread_large_mapped_frame_at(next).unwrap().is_none());
    assert!(cursor
        .reread_large_mapped_frame_at(u64::MAX)
        .unwrap()
        .is_none());
}

#[test]
fn mapped_cursor_stops_at_torn_header_payload_and_crc_tail() {
    let dir = tempfile::tempdir().unwrap();
    let header_path = dir.path().join("torn-header.log");
    std::fs::write(&header_path, 4u64.to_le_bytes()).unwrap();
    let mut header_cursor = FramedLogCursor::open(&header_path).unwrap();
    assert!(header_cursor.next_mapped_frame().unwrap().is_none());

    let payload_path = dir.path().join("torn-payload.log");
    let mut payload_writer = FramedLogWriter::open(&payload_path, FsyncPolicy::Always).unwrap();
    payload_writer.append(1, b"clean prefix").unwrap();
    payload_writer.sync().unwrap();
    let mut tail = OpenOptions::new().append(true).open(&payload_path).unwrap();
    tail.write_all(&2u64.to_le_bytes()).unwrap();
    tail.write_all(&3u32.to_le_bytes()).unwrap();
    tail.write_all(&0u32.to_le_bytes()).unwrap();
    tail.write_all(b"to").unwrap();
    tail.sync_all().unwrap();
    let mut payload_cursor = FramedLogCursor::open(&payload_path).unwrap();
    assert_eq!(payload_cursor.next_mapped_frame().unwrap().unwrap().seq, 1);
    assert!(payload_cursor.next_mapped_frame().unwrap().is_none());

    let crc_path = dir.path().join("crc.log");
    let mut crc_writer = FramedLogWriter::open(&crc_path, FsyncPolicy::Always).unwrap();
    crc_writer.append(3, b"bad crc payload").unwrap();
    crc_writer.sync().unwrap();
    let mut corrupt = OpenOptions::new().write(true).open(&crc_path).unwrap();
    corrupt.seek(SeekFrom::Start(HEADER_LEN as u64)).unwrap();
    corrupt.write_all(b"!").unwrap();
    corrupt.sync_all().unwrap();
    let mut crc_cursor = FramedLogCursor::open(&crc_path).unwrap();
    assert!(crc_cursor.next_mapped_frame().unwrap().is_none());
}

#[test]
fn mapped_cursor_rejects_oversized_and_overflow_offsets_without_owned_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized.log");
    let mut file = File::create(&path).unwrap();
    file.write_all(&9u64.to_le_bytes()).unwrap();
    file.write_all(&u32::MAX.to_le_bytes()).unwrap();
    file.write_all(&0u32.to_le_bytes()).unwrap();
    file.sync_all().unwrap();
    let mut cursor = FramedLogCursor::open(&path).unwrap();
    let error = cursor.next_mapped_frame().unwrap_err();
    assert!(error.to_string().contains("oversized legacy log frame"));
    assert!(cursor
        .reread_large_mapped_frame_at(u64::MAX)
        .unwrap()
        .is_none());
}

#[test]
fn mapped_cursor_never_allocates_a_payload_sized_transient_buffer() {
    const PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
    const MAX_ALLOWED_REQUEST: usize = 64 * 1024;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.log");
    let payload = vec![0x5a; PAYLOAD_BYTES];
    let mut writer = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    writer.append(44, &payload).unwrap();
    writer.sync().unwrap();
    drop(writer);
    let mut cursor = FramedLogCursor::open(&path).unwrap();
    let start = cursor.byte_offset();

    let observation = AllocationObservation::start();
    let view = cursor.next_mapped_frame().unwrap().unwrap();
    assert_eq!(view.seq, 44);
    assert_eq!(view.payload(), payload.as_slice());
    let next_largest = observation.largest_request();
    drop(view);
    observation.reset();
    let reread = cursor.reread_large_mapped_frame_at(start).unwrap().unwrap();
    assert_eq!(reread.seq, 44);
    assert_eq!(reread.payload(), payload.as_slice());
    let reread_largest = observation.largest_request();
    drop(reread);
    drop(observation);

    assert!(next_largest <= MAX_ALLOWED_REQUEST);
    assert!(reread_largest <= MAX_ALLOWED_REQUEST);
    assert!(next_largest < PAYLOAD_BYTES);
    assert!(reread_largest < PAYLOAD_BYTES);
}
