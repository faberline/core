use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use openapi_codegen::{generate, GenOptions, GeneratedOutput, Lang};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;

fn opts() -> GenOptions {
    GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_emit_client(false)
        .with_emit_hooks(false)
}

// A valid OpenAPI 3.1 document with one object schema and one string property.
fn spec_text(title: &str, schema: &str, prop_name: &str) -> String {
    let body = serde_json::json!({
        "type": "object",
        "properties": { (prop_name): { "type": "string" } },
        "required": [prop_name]
    });
    let mut schemas = serde_json::Map::new();
    schemas.insert(schema.to_string(), body);
    serde_json::json!({
        "openapi": "3.1.0",
        "info": { "title": title, "version": "1.0.0" },
        "paths": {},
        "components": { "schemas": schemas }
    })
    .to_string()
}

fn files(output: &GeneratedOutput) -> Vec<(String, String)> {
    output
        .files
        .iter()
        .map(|f| (f.rel_path.clone(), f.contents.clone()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_text_never_panics(text in ".{0,200}") {
        let _ = generate(&text, &opts());
    }

    #[test]
    fn valid_spec_generates_and_is_stable(
        title in "[A-Za-z]{1,12}",
        schema in "[A-Z][a-z]{2,8}",
        prop_name in "[a-z]{1,8}",
    ) {
        let spec = spec_text(&title, &schema, &prop_name);
        let first = generate(&spec, &opts());
        let second = generate(&spec, &opts());
        prop_assert!(first.is_ok());
        prop_assert!(second.is_ok());
        prop_assert_eq!(files(&first.expect("checked above")), files(&second.expect("checked above")));
    }
}

// Counts its calls, so the same spec gives a different result each time.
// It is a deliberately broken generator, used only by the negative test below.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn broken_generate(spec: &str) -> usize {
    generate(spec, &opts()).expect("valid spec").files.len()
        + COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn valid_spec_text(title: String) -> String {
    spec_text(&title, "Pet", "name")
}

#[test]
fn seeded_bug_is_found() {
    let mut runner = TestRunner::default();
    let result = runner.run(&"[A-Za-z]{1,8}".prop_map(valid_spec_text), |s| {
        prop_assert_eq!(broken_generate(&s), broken_generate(&s));
        Ok(())
    });
    assert!(result.is_err());
}
