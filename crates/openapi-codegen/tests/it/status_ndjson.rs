use std::path::Path;

fn read(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
        .unwrap_or_else(|e| panic!("read {name}: {e}"))
}

fn line_containing<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle}"))
}

#[test]
fn streaming_row_is_limited() {
    let status = read("STATUS.md");
    let row = line_containing(&status, "`streaming-operation-emission`");
    assert!(row.contains("| Limited |"), "row: {row}");
    assert!(!row.contains("Not supported"), "row: {row}");
}

#[test]
fn streaming_row_names_the_proving_tests() {
    let status = read("STATUS.md");
    let row = line_containing(&status, "`streaming-operation-emission`");
    for needle in [
        "ndjson_upload_method_streams_body_lines",
        "ndjson_upload_method_takes_async_iterable",
        "generated_rust_client_streams_ndjson_records",
        "cargo test -p openapi-codegen",
    ] {
        assert!(row.contains(needle), "row misses {needle}: {row}");
    }
}

#[test]
fn json_model_row_mentions_ndjson() {
    let status = read("STATUS.md");
    let row = line_containing(&status, "`json-operation-model`");
    assert!(row.contains("NDJSON"), "row: {row}");
    assert!(
        !row.contains("streaming bodies are not represented"),
        "row: {row}"
    );
}

#[test]
fn readme_no_longer_denies_streaming() {
    let readme = read("README.md");
    assert!(
        !readme.contains("It does not model streaming operations"),
        "README still denies streaming"
    );
    assert!(readme.contains("NDJSON"), "README does not mention NDJSON");
}

#[test]
fn roadmap_keeps_the_stable_id() {
    let roadmap = read("ROADMAP.md");
    assert!(
        roadmap.contains("ID: `media-type-and-streaming-operations`"),
        "ROADMAP lost the stable id"
    );
    assert!(
        roadmap.contains("Remaining:"),
        "ROADMAP has no Remaining: bullet"
    );
}
