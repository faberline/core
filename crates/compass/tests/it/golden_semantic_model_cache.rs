//! Golden bytes for the semantic-model disk cache (compass E3).
//!
//! `DiskCache` writes one bincode `PersistedEntry` per source file: the schema
//! version, the content hash, the `SemanticModel` (with its `ScopeId` and
//! `SymbolId`s) and the file's diagnostics. These literals pin today's bytes so
//! that the id newtypes and the `RuleCode` change keep old caches readable.

use std::path::{Path, PathBuf};

use compass::diagnostic::TextEdit;
use compass::server::disk_cache::DiskCache;
use compass::type_inference::{SemanticModel, SemanticSymbolKind, SymbolData, TypeInfo};
use compass::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};

#[track_caller]
fn pin_text(actual: &str, expected: &str) {
    assert_eq!(actual, expected);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn range(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
    Range::new(Position::new(sl, sc), Position::new(el, ec))
}

/// One scope, one symbol, one reference and one typed range: single entries
/// keep the model's hash maps in a fixed order.
fn model() -> SemanticModel {
    let mut model = SemanticModel::new();
    let scope = model.add_scope(None, range(0, 0, 3, 0));
    let symbol = model.add_symbol(SymbolData {
        name: "greet".to_string(),
        kind: SemanticSymbolKind::Function,
        def_range: range(0, 4, 0, 9),
        file_path: PathBuf::from("/w/app.py"),
        type_info: TypeInfo::Str,
        documentation: Some("Say hi.".to_string()),
        scope_id: scope,
        parent_id: None,
    });
    model.add_reference(symbol, range(2, 0, 2, 5));
    model.add_typed_range(range(2, 0, 2, 5), TypeInfo::Str, Some(symbol));
    model
}

/// bincode is not self-describing, and `quick_fixes` is skipped when empty,
/// so only a diagnostic with a fix round-trips through the cache.
fn diagnostics() -> Vec<Diagnostic> {
    vec![Diagnostic::new(
        range(2, 0, 2, 5),
        DiagnosticSeverity::Warning,
        "PY010",
        DiagnosticCategory::Names,
        "shadowed name",
    )
    .with_fix(
        "Rename",
        vec![TextEdit {
            range: range(2, 0, 2, 5),
            new_text: "greeting".to_string(),
        }],
    )]
}

const CONTENT_HASH: u64 = 0x0102_0304_0506_0708;

fn only_idx_file(dir: &Path) -> Vec<u8> {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "idx"))
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    std::fs::read(&files[0]).unwrap()
}

#[tokio::test]
async fn disk_cache_entry_bytes_are_pinned() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cache");
    let source = PathBuf::from("/w/app.py");
    let (model, diagnostics) = (model(), diagnostics());

    let cache = DiskCache::new(cache_dir.clone());
    cache
        .store(&source, CONTENT_HASH, &model, &diagnostics)
        .await;
    let bytes = only_idx_file(&cache_dir);
    pin_text(&hex(&bytes), "0100000008070605040302010100000000000000000000000000000005000000000000006772656574010000000000000004000000000000000900000009000000000000002f772f6170702e7079060000000107000000000000005361792068692e00000000000000000002000000000000000000000000000000000000000400000000000000090000000100000000000000000200000000000000020000000500000000010000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000100000000000000020000000000000002000000050000000600000001000000000000000001000000000000000500000000000000677265657401000000000000000000000000000000010000000000000001000000000000000100000000000000020000000000000002000000050000000100000005000000000000005059303130020000000d00000000000000736861646f776564206e616d650100000000000000060000000000000052656e616d6501000000000000000200000000000000020000000500000008000000000000006772656574696e67");

    // The entry is the tuple (version, content hash, model, diagnostics).
    let tuple = (1u32, CONTENT_HASH, &model, &diagnostics);
    assert_eq!(bincode::serialize(&tuple).unwrap(), bytes);
    let (version, hash, decoded_model, decoded_diags): (u32, u64, SemanticModel, Vec<Diagnostic>) =
        bincode::deserialize(&bytes).unwrap();
    assert_eq!((version, hash), (1, CONTENT_HASH));
    assert_eq!(format!("{decoded_model:?}"), format!("{model:?}"));
    assert_eq!(format!("{decoded_diags:?}"), format!("{diagnostics:?}"));

    let loaded = cache.load(&source, CONTENT_HASH).await.unwrap();
    assert_eq!(loaded.content_hash, CONTENT_HASH);
    assert_eq!(format!("{:?}", loaded.semantic_model), format!("{model:?}"));
    assert_eq!(
        format!("{:?}", loaded.diagnostics),
        format!("{diagnostics:?}")
    );
}

#[test]
fn semantic_model_json_bytes_are_pinned() {
    let model = model();
    let json = serde_json::to_string(&model).unwrap();
    pin_text(&json, "{\"symbols\":{\"0\":{\"name\":\"greet\",\"kind\":\"Function\",\"def_range\":{\"start\":{\"line\":0,\"character\":4},\"end\":{\"line\":0,\"character\":9}},\"file_path\":\"/w/app.py\",\"type_info\":\"Str\",\"documentation\":\"Say hi.\",\"scope_id\":0,\"parent_id\":null}},\"references\":[{\"symbol_id\":0,\"range\":{\"start\":{\"line\":0,\"character\":4},\"end\":{\"line\":0,\"character\":9}},\"is_definition\":true},{\"symbol_id\":0,\"range\":{\"start\":{\"line\":2,\"character\":0},\"end\":{\"line\":2,\"character\":5}},\"is_definition\":false}],\"scopes\":{\"0\":{\"id\":0,\"parent\":null,\"range\":{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":3,\"character\":0}},\"symbols\":[]}},\"typed_ranges\":[{\"range\":{\"start\":{\"line\":2,\"character\":0},\"end\":{\"line\":2,\"character\":5}},\"type_info\":\"Str\",\"symbol_id\":0}],\"name_to_symbols\":{\"greet\":[0]},\"next_symbol_id\":1,\"next_scope_id\":1}");
    let decoded: SemanticModel = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{decoded:?}"), format!("{model:?}"));
}

#[test]
fn diagnostic_without_fixes_bincode_bytes_are_pinned() {
    let diagnostic = Diagnostic::new(
        range(1, 2, 3, 4),
        DiagnosticSeverity::Error,
        "RS001",
        DiagnosticCategory::Syntax,
        "syntax error",
    );
    pin_text(&hex(&bincode::serialize(&diagnostic).unwrap()), "010000000200000003000000040000000000000005000000000000005253303031000000000c0000000000000073796e746178206572726f72");
}
