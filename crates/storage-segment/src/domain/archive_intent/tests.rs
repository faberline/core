use super::{ArchiveIntent, ARCHIVE_INTENT_FORMAT_VERSION};
use crate::domain::{ArchiveObject, SegmentError};

const SEGMENT_SHA256: &str = "03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f";

const INTENT_JSON: &str = r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","size":7,"sha256":"03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f","content_type":"application/vnd.apache.parquet"}"#;

fn intent() -> ArchiveIntent {
    ArchiveIntent::new(
        "archive/segment-0001.parquet",
        7,
        SEGMENT_SHA256,
        "application/vnd.apache.parquet",
    )
}

#[test]
fn intent_json_is_pinned() {
    assert_eq!(ARCHIVE_INTENT_FORMAT_VERSION, 1);
    assert_eq!(intent().encode().unwrap(), INTENT_JSON.as_bytes());
    assert_eq!(
        ArchiveIntent::decode(INTENT_JSON.as_bytes()).unwrap(),
        intent()
    );
}

#[test]
fn for_object_hashes_the_bytes() {
    let object = ArchiveObject::new(
        "archive/segment-0001.parquet",
        b"segment".to_vec(),
        "application/vnd.apache.parquet",
    );
    assert_eq!(ArchiveIntent::for_object(&object), intent());
}

#[test]
fn object_key_is_prefix_plus_hash_of_the_segment_key() {
    assert_eq!(
        intent().object_key(),
        "_archive/intent/fb6c3989579d2eae9aff55014178ae1764a8ef8b1ad93c0294d46d8ce7fac5b2.json"
    );
}

#[test]
fn decode_refuses_a_wrong_version() {
    let text = INTENT_JSON.replace("\"format_version\":1", "\"format_version\":2");
    assert!(matches!(
        ArchiveIntent::decode(text.as_bytes()),
        Err(SegmentError::UnsupportedArchiveIntentVersion { version: 2 })
    ));
}

#[test]
fn decode_refuses_a_truncated_record() {
    assert!(matches!(
        ArchiveIntent::decode(&INTENT_JSON.as_bytes()[..40]),
        Err(SegmentError::CorruptArchiveIntent { .. })
    ));
    assert!(matches!(
        ArchiveIntent::decode(b""),
        Err(SegmentError::CorruptArchiveIntent { .. })
    ));
}

#[test]
fn decode_refuses_a_missing_version() {
    assert!(matches!(
        ArchiveIntent::decode(br#"{"segment_key":"a"}"#),
        Err(SegmentError::CorruptArchiveIntent { .. })
    ));
}
