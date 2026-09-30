use super::{ObjectMeta, ObjectVersion};

const FULL_META_JSON: &str = r#"{"key":"archive/2026/segment.parquet","size":4096,"content_type":"application/vnd.apache.parquet","version":"1712345678901234","etag":"CJLx8r7T6YQDEAE=","updated":"2026-01-02T03:04:05.678Z"}"#;

const BARE_META_JSON: &str = r#"{"key":"catalog/pages/a.json","size":0,"content_type":"application/json","version":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","etag":null,"updated":null}"#;

fn full_meta() -> ObjectMeta {
    ObjectMeta {
        key: "archive/2026/segment.parquet".to_string(),
        size: 4096,
        content_type: "application/vnd.apache.parquet".to_string(),
        version: ObjectVersion::new("1712345678901234"),
        etag: Some("CJLx8r7T6YQDEAE=".to_string()),
        updated: Some("2026-01-02T03:04:05.678Z".to_string()),
    }
}

fn bare_meta() -> ObjectMeta {
    ObjectMeta {
        key: "catalog/pages/a.json".to_string(),
        size: 0,
        content_type: "application/json".to_string(),
        version: ObjectVersion::new(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        etag: None,
        updated: None,
    }
}

#[test]
fn object_meta_json_is_pinned() {
    assert_eq!(serde_json::to_string(&full_meta()).unwrap(), FULL_META_JSON);
    assert_eq!(
        serde_json::from_str::<ObjectMeta>(FULL_META_JSON).unwrap(),
        full_meta()
    );
}

#[test]
fn object_meta_without_etag_or_update_time_writes_nulls() {
    assert_eq!(serde_json::to_string(&bare_meta()).unwrap(), BARE_META_JSON);
    assert_eq!(
        serde_json::from_str::<ObjectMeta>(BARE_META_JSON).unwrap(),
        bare_meta()
    );
}
