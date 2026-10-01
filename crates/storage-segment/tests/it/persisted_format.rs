//! Golden bytes for the persisted catalog and archive formats. sift stores
//! `CatalogRoot` in its archive manifest, and a page key is the sha256 of the
//! page JSON, so these bytes must not change.

use std::sync::Arc;

use storage_object::LocalObjectStore;
use storage_segment::{
    ArchiveCommit, ArchiveCoordinator, ArchiveObject, ArchivedObject, CatalogEntry, CatalogPageRef,
    CatalogRoot, PagedCatalog,
};

const ROOT_JSON: &str = r#"{"format_version":1,"height":2,"entry_count":120000,"page_bytes_limit":65536,"root":{"key":"sift/logs/catalog/pages/5f0c8f8a2b7e4d1c9a3b6e5d4c3b2a1908f7e6d5c4b3a2918f7e6d5c4b3a2918.json","sha256":"5f0c8f8a2b7e4d1c9a3b6e5d4c3b2a1908f7e6d5c4b3a2918f7e6d5c4b3a2918","bytes":1893,"entry_count":120000,"first_key":"segment/000000000","last_key":"segment/000119999"}}"#;

const SMALL_ROOT_JSON: &str = r#"{"format_version":1,"height":0,"entry_count":2,"page_bytes_limit":65536,"root":{"key":"golden/catalog/pages/45952450e3d863b15647f40b4e467b020c6fd8a19174f230dbdb6134c98b5164.json","sha256":"45952450e3d863b15647f40b4e467b020c6fd8a19174f230dbdb6134c98b5164","bytes":106,"entry_count":2,"first_key":"logs/a","last_key":"logs/b"}}"#;

const SMALL_LEAF_PAGE_JSON: &str = r#"{"format_version":1,"kind":"leaf","entries":[{"key":"logs/a","value":[49]},{"key":"logs/b","value":[50]}]}"#;

const LOOKUP_ENTRY_JSON: &str = r#"{"key":"logs/a","value":[49]}"#;

const TREE_ROOT_JSON: &str = r#"{"format_version":1,"height":1,"entry_count":600,"page_bytes_limit":4096,"root":{"key":"golden/tree/pages/22afdde39bf388ad9031efe488d0cfaa3e9b36822b6816aba86fa2a4bf9a23f5.json","sha256":"22afdde39bf388ad9031efe488d0cfaa3e9b36822b6816aba86fa2a4bf9a23f5","bytes":2452,"entry_count":600,"first_key":"segment/000000000","last_key":"segment/000000599"}}"#;

const ARCHIVED_OBJECT_JSON: &str = r#"{"key":"archive/segment-0001.parquet","size":7,"content_type":"application/vnd.apache.parquet","sha256":"03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f","version":"03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f"}"#;

const ARCHIVE_COMMIT_JSON: &str = r#"{"objects":[{"key":"archive/segment-0001.parquet","size":7,"content_type":"application/vnd.apache.parquet","sha256":"03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f","version":"03e71c6d7dc6bd4e89ceaf32f6488ca0aa69b0de784b706cac5818d680e9d97f"}],"manifest":{"key":"archive/manifest.json","size":14,"content_type":"application/json","sha256":"dfb4e167c75bcd060125723c543409d3134c992871d5da653e75eae8d5276b4b","version":"dfb4e167c75bcd060125723c543409d3134c992871d5da653e75eae8d5276b4b"}}"#;

fn root() -> CatalogRoot {
    CatalogRoot {
        format_version: 1,
        height: 2,
        entry_count: 120_000,
        page_bytes_limit: 65_536,
        root: CatalogPageRef {
            key: "sift/logs/catalog/pages/5f0c8f8a2b7e4d1c9a3b6e5d4c3b2a1908f7e6d5c4b3a2918f7e6d5c4b3a2918.json".to_string(),
            sha256: "5f0c8f8a2b7e4d1c9a3b6e5d4c3b2a1908f7e6d5c4b3a2918f7e6d5c4b3a2918".to_string(),
            bytes: 1_893,
            entry_count: 120_000,
            first_key: "segment/000000000".to_string(),
            last_key: "segment/000119999".to_string(),
        },
    }
}

fn entry(key: &str, value: &[u8]) -> CatalogEntry {
    serde_json::from_value(serde_json::json!({ "key": key, "value": value })).unwrap()
}

#[test]
fn catalog_root_and_page_ref_json_is_pinned() {
    assert_eq!(serde_json::to_string(&root()).unwrap(), ROOT_JSON);
    assert_eq!(
        serde_json::from_str::<CatalogRoot>(ROOT_JSON).unwrap(),
        root()
    );
}

#[test]
fn a_built_catalog_writes_pinned_leaf_page_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let catalog = PagedCatalog::new(store, "golden/catalog").unwrap();
    let mutation = catalog
        .build([entry("logs/b", b"2"), entry("logs/a", b"1")])
        .unwrap();

    assert_eq!(
        serde_json::to_string(&mutation.root).unwrap(),
        SMALL_ROOT_JSON
    );
    assert_eq!(
        serde_json::from_str::<CatalogRoot>(SMALL_ROOT_JSON).unwrap(),
        mutation.root
    );
    let page = std::fs::read(dir.path().join(&mutation.root.root.key)).unwrap();
    assert_eq!(String::from_utf8(page).unwrap(), SMALL_LEAF_PAGE_JSON);

    let found = catalog.lookup(&mutation.root, "logs/a").unwrap().unwrap();
    assert_eq!(serde_json::to_string(&found).unwrap(), LOOKUP_ENTRY_JSON);
    assert_eq!(
        serde_json::from_str::<CatalogEntry>(LOOKUP_ENTRY_JSON).unwrap(),
        found
    );
}

#[test]
fn a_built_catalog_tree_has_a_pinned_root() {
    // The root reference carries the sha256 of the branch page, which in turn
    // names every leaf page by its sha256, so this pins every page's bytes.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let catalog = PagedCatalog::with_page_bytes(store, "golden/tree", 4 * 1024).unwrap();
    let entries =
        (0..600_u64).map(|index| entry(&format!("segment/{index:09}"), &index.to_le_bytes()));
    let mutation = catalog.build(entries).unwrap();

    assert_eq!(
        serde_json::to_string(&mutation.root).unwrap(),
        TREE_ROOT_JSON
    );
    assert_eq!(
        serde_json::from_str::<CatalogRoot>(TREE_ROOT_JSON).unwrap(),
        mutation.root
    );
}

#[test]
fn archive_receipts_json_is_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalObjectStore::open(dir.path()).unwrap());
    let coordinator = ArchiveCoordinator::new(store);
    let mut transaction = coordinator.begin();
    let archived = transaction
        .put(ArchiveObject::new(
            "archive/segment-0001.parquet",
            b"segment".to_vec(),
            "application/vnd.apache.parquet",
        ))
        .unwrap();
    let commit = transaction
        .commit(ArchiveObject::new(
            "archive/manifest.json",
            br#"{"segments":1}"#.to_vec(),
            "application/json",
        ))
        .unwrap();

    assert_eq!(
        serde_json::to_string(&archived).unwrap(),
        ARCHIVED_OBJECT_JSON
    );
    assert_eq!(
        serde_json::from_str::<ArchivedObject>(ARCHIVED_OBJECT_JSON).unwrap(),
        archived
    );
    assert_eq!(serde_json::to_string(&commit).unwrap(), ARCHIVE_COMMIT_JSON);
    assert_eq!(
        serde_json::from_str::<ArchiveCommit>(ARCHIVE_COMMIT_JSON).unwrap(),
        commit
    );
}
