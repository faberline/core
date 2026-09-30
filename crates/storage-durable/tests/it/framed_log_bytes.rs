//! Pins the framed-log bytes: a 16-byte header (`seq` u64 LE, `len` u32 LE,
//! CRC-32 of the payload u32 LE) and then the payload, for each frame.

use storage_durable::{FramedLogReader, FramedLogWriter, FsyncPolicy};

/// Frame `(7, b"alpha")` followed by frame `(0x0102030405060708, b"")`.
const TWO_FRAMES_HEX: &str = concat!(
    // seq 7, len 5, CRC-32 0xd0e0396a, "alpha"
    "0700000000000000",
    "05000000",
    "6a39e0d0",
    "616c706861",
    // seq 0x0102030405060708, len 0, CRC-32 0, no payload
    "0807060504030201",
    "00000000",
    "00000000",
);

/// How `FramedLogReader::read_frames` returns the two frames.
const TWO_FRAMES_DEBUG: &str = "[LogFrame { seq: 7, payload: [97, 108, 112, 104, 97] }, \
     LogFrame { seq: 72623859790382856, payload: [] }]";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn two_frames_encode_to_the_pinned_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pinned.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(7, b"alpha").unwrap();
    log.append(0x0102_0304_0506_0708, b"").unwrap();
    log.sync().unwrap();
    drop(log);

    assert_eq!(hex(&std::fs::read(&path).unwrap()), TWO_FRAMES_HEX);
}

#[test]
fn the_pinned_bytes_decode_to_two_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pinned.log");
    std::fs::write(&path, unhex(TWO_FRAMES_HEX)).unwrap();

    let frames = FramedLogReader::read_frames(&path, 0).unwrap();
    assert_eq!(format!("{frames:?}"), TWO_FRAMES_DEBUG);

    let mut cursor = storage_durable::FramedLogCursor::open(&path).unwrap();
    let first = cursor.next_mapped_frame().unwrap().unwrap();
    assert_eq!((first.seq, first.payload()), (7, &b"alpha"[..]));
    let second = cursor.next_mapped_frame().unwrap().unwrap();
    assert_eq!(
        (second.seq, second.payload()),
        (0x0102_0304_0506_0708, &b""[..])
    );
    assert!(cursor.next_mapped_frame().unwrap().is_none());
}
