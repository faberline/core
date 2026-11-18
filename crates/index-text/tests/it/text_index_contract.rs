use std::collections::BTreeMap;

use index_text::{
    Analyzer, DocumentId, DocumentVersion, FieldSpec, MatchOperator, MemoryTextIndex, TextDocument,
    TextIndex, TextQuery, TextSchema,
};

fn schema() -> TextSchema {
    TextSchema::new(BTreeMap::from([
        (
            "body".to_string(),
            FieldSpec::text(Analyzer::WhitespaceLower),
        ),
        ("project".to_string(), FieldSpec::keyword()),
    ]))
    .unwrap()
}

fn document(id: &str, version: u64, body: &str, project: &str) -> TextDocument {
    TextDocument::new(DocumentId::new(id), DocumentVersion::new(version))
        .with_field("body", body)
        .with_field("project", project)
}

fn find_error_text(index: &impl TextIndex) -> Vec<String> {
    index
        .search(
            &TextQuery::match_text("body", "disk error", MatchOperator::All),
            10,
        )
        .unwrap()
        .into_iter()
        .map(|hit| hit.external_id.to_string())
        .collect()
}

#[test]
fn schema_document_query_snapshot_and_rebuild_are_product_neutral() {
    let index = MemoryTextIndex::new(schema()).unwrap();
    index
        .upsert(document("log-1", 1, "Disk error on shard seven", "alpha"))
        .unwrap();
    index
        .upsert(document("log-2", 2, "HTTP request completed", "alpha"))
        .unwrap();
    assert_eq!(find_error_text(&index), vec!["log-1"]);

    let snapshot = index.snapshot().unwrap();
    let restored = MemoryTextIndex::new(schema()).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(find_error_text(&restored), vec!["log-1"]);

    restored
        .rebuild(vec![document(
            "log-3",
            3,
            "Disk error after rebuild",
            "beta",
        )])
        .unwrap();
    assert_eq!(find_error_text(&restored), vec!["log-3"]);
}

#[test]
fn older_document_versions_cannot_replace_newer_index_state() {
    let index = MemoryTextIndex::new(schema()).unwrap();
    index
        .upsert(document("log-1", 9, "new durable value", "alpha"))
        .unwrap();
    index
        .upsert(document("log-1", 8, "stale value", "alpha"))
        .unwrap();

    let hits = index
        .search(
            &TextQuery::match_text("body", "durable", MatchOperator::All),
            10,
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].version, DocumentVersion::new(9));
}

#[test]
fn versioned_delete_blocks_stale_replay_and_allows_a_newer_document() {
    let index = MemoryTextIndex::new(schema()).unwrap();
    index
        .upsert(document("log-1", 5, "value before delete", "alpha"))
        .unwrap();
    assert!(index
        .delete(&DocumentId::new("log-1"), Some(DocumentVersion::new(9)))
        .unwrap());

    index
        .upsert(document("log-1", 8, "stale replay", "alpha"))
        .unwrap();
    assert!(index.search(&TextQuery::All, 10).unwrap().is_empty());

    index
        .upsert(document("log-1", 10, "new value", "alpha"))
        .unwrap();
    let hits = index.search(&TextQuery::All, 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].version, DocumentVersion::new(10));
}

#[test]
fn delete_tombstone_survives_snapshot_restore() {
    let index = MemoryTextIndex::new(schema()).unwrap();
    index
        .upsert(document("log-1", 5, "value before delete", "alpha"))
        .unwrap();
    assert!(index
        .delete(&DocumentId::new("log-1"), Some(DocumentVersion::new(9)))
        .unwrap());

    let encoded = index.snapshot().unwrap().encode().unwrap();
    let snapshot = index_text::TextIndexSnapshot::decode(&encoded).unwrap();
    let restored = MemoryTextIndex::new(schema()).unwrap();
    restored.restore(&snapshot).unwrap();
    restored
        .upsert(document("log-1", 8, "stale after restart", "alpha"))
        .unwrap();

    assert!(restored.search(&TextQuery::All, 10).unwrap().is_empty());
}

#[test]
fn maximum_version_tombstone_blocks_replay_after_restore() {
    let index = MemoryTextIndex::new(schema()).unwrap();
    index
        .upsert(document("log-1", 1, "first value", "alpha"))
        .unwrap();
    assert!(index
        .delete(
            &DocumentId::new("log-1"),
            Some(DocumentVersion::new(u64::MAX))
        )
        .unwrap());

    let bytes = index.snapshot().unwrap().encode().unwrap();
    assert!(
        String::from_utf8_lossy(&bytes).contains(r#""tombstones":{"log-1":18446744073709551615}"#)
    );
    let restored = MemoryTextIndex::new(schema()).unwrap();
    restored
        .restore(&index_text::TextIndexSnapshot::decode(&bytes).unwrap())
        .unwrap();
    restored
        .upsert(document("log-1", u64::MAX, "stale value", "alpha"))
        .unwrap();
    assert!(restored.search(&TextQuery::All, 10).unwrap().is_empty());
}
