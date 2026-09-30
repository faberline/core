//! `LogFrame` is built with `new` and read through its accessors, and a frame
//! built that way equals the one the reader returns for the same bytes.

use storage_durable::{FramedLogReader, FramedLogWriter, FsyncPolicy, LogFrame};

#[test]
fn a_new_frame_reads_back_its_parts() {
    let frame = LogFrame::new(7, b"alpha".to_vec());
    assert_eq!(frame.seq(), 7);
    assert_eq!(frame.payload(), b"alpha");
    assert_eq!(frame.into_payload(), b"alpha".to_vec());
}

#[test]
fn a_new_frame_equals_the_frame_read_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frames.log");
    let mut log = FramedLogWriter::open(&path, FsyncPolicy::Always).unwrap();
    log.append(7, b"alpha").unwrap();
    log.append(8, b"").unwrap();
    log.sync().unwrap();
    drop(log);

    assert_eq!(
        FramedLogReader::read_frames(&path, 0).unwrap(),
        [
            LogFrame::new(7, b"alpha".to_vec()),
            LogFrame::new(8, Vec::new())
        ]
    );
}
