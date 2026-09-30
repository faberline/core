use super::CatalogEntry;

const ENTRY_JSON: &str = r#"{"key":"segment/000000042","value":[42,0,0,0,0,0,0,0]}"#;

const EMPTY_VALUE_JSON: &str = r#"{"key":"logs/2026-01-02","value":[]}"#;

fn entry() -> CatalogEntry {
    CatalogEntry {
        key: "segment/000000042".to_string(),
        value: 42_u64.to_le_bytes().to_vec(),
    }
}

fn empty_value_entry() -> CatalogEntry {
    CatalogEntry {
        key: "logs/2026-01-02".to_string(),
        value: Vec::new(),
    }
}

#[test]
fn catalog_entry_json_is_pinned() {
    assert_eq!(serde_json::to_string(&entry()).unwrap(), ENTRY_JSON);
    assert_eq!(
        serde_json::from_str::<CatalogEntry>(ENTRY_JSON).unwrap(),
        entry()
    );
}

#[test]
fn catalog_entry_with_an_empty_value_is_pinned() {
    assert_eq!(
        serde_json::to_string(&empty_value_entry()).unwrap(),
        EMPTY_VALUE_JSON
    );
    assert_eq!(
        serde_json::from_str::<CatalogEntry>(EMPTY_VALUE_JSON).unwrap(),
        empty_value_entry()
    );
}
