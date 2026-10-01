use std::collections::BTreeMap;

use super::{TextIndexSnapshot, SNAPSHOT_FORMAT_VERSION};
use crate::domain::document::TextDocument;
use crate::domain::schema::{Analyzer, FieldSpec, TextSchema};
use crate::{DocumentId, DocumentVersion};

const SNAPSHOT_JSON: &str = r#"{"format_version":1,"schema":{"fields":{"body":{"kind":{"type":"text","analyzer":"whitespace_lower"}},"tag":{"kind":{"type":"keyword"}}}},"documents":[{"external_id":"doc-1","version":3,"fields":{"body":"hello world","tag":"greeting"}}],"tombstones":{"doc-9":7}}"#;

/// Version 1 snapshots written before tombstones existed have no
/// `tombstones` key, and a snapshot with no tombstones still writes none.
const SNAPSHOT_WITHOUT_TOMBSTONES_JSON: &str = r#"{"format_version":1,"schema":{"fields":{"body":{"kind":{"type":"text","analyzer":"whitespace_lower"}},"tag":{"kind":{"type":"keyword"}}}},"documents":[{"external_id":"doc-1","version":3,"fields":{"body":"hello world","tag":"greeting"}}]}"#;

fn snapshot(tombstones: BTreeMap<DocumentId, DocumentVersion>) -> TextIndexSnapshot {
    let schema = TextSchema::new(BTreeMap::from([
        (
            "body".to_string(),
            FieldSpec::text(Analyzer::WhitespaceLower),
        ),
        ("tag".to_string(), FieldSpec::keyword()),
    ]))
    .unwrap();
    TextIndexSnapshot {
        format_version: SNAPSHOT_FORMAT_VERSION,
        schema,
        documents: vec![
            TextDocument::new(DocumentId::new("doc-1"), DocumentVersion::new(3))
                .with_field("tag", "greeting")
                .with_field("body", "hello world"),
        ],
        tombstones,
    }
}

#[test]
fn text_index_snapshot_bytes_are_pinned() {
    let snapshot = snapshot(BTreeMap::from([(
        DocumentId::new("doc-9"),
        DocumentVersion::new(7),
    )]));
    assert_eq!(snapshot.encode().unwrap(), SNAPSHOT_JSON.as_bytes());
    assert_eq!(
        TextIndexSnapshot::decode(SNAPSHOT_JSON.as_bytes()).unwrap(),
        snapshot
    );
}

#[test]
fn text_index_snapshot_without_tombstones_omits_the_key() {
    let snapshot = snapshot(BTreeMap::new());
    assert_eq!(
        snapshot.encode().unwrap(),
        SNAPSHOT_WITHOUT_TOMBSTONES_JSON.as_bytes()
    );
    assert_eq!(
        TextIndexSnapshot::decode(SNAPSHOT_WITHOUT_TOMBSTONES_JSON.as_bytes()).unwrap(),
        snapshot
    );
}
