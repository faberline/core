use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{generate, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Optional untyped field", "version": "1.0.0" },
  "paths": {},
  "components": { "schemas": {
    "Label": { "type": "string" },
    "Holder": {
      "type": "object",
      "required": ["name"],
      "properties": {
        "name": { "type": "string" },
        "payload": {},
        "label": { "$ref": "#/components/schemas/Label" }
      }
    }
  } }
}"##;

fn opts() -> GenOptions {
    GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_emit_hooks(false)
        .with_target(TargetProfile::Rust(RustTarget::Rust2021))
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp directory");
    path
}

fn file<'a>(output: &'a GeneratedOutput, name: &str) -> &'a str {
    &output
        .files
        .iter()
        .find(|file| file.rel_path == name)
        .unwrap_or_else(|| panic!("generated file {name}"))
        .contents
}

fn assert_success(output: Output, description: &str) {
    assert!(
        output.status.success(),
        "{description}: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn optional_untyped_property_compiles_and_round_trips() {
    let output = generate(SPEC, &opts()).expect("generate Rust models");
    let models = file(&output, "models.rs");

    let root = temp_dir("optional-untyped-field");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create generated source directory");
    // The models module is the whole crate root; `client.rs` needs reqwest and is not built here.
    fs::write(src.join("lib.rs"), models).expect("write generated models as lib.rs");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "optional_untyped_field"
version = "0.0.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    )
    .expect("write generated crate manifest");

    let tests = root.join("tests");
    fs::create_dir_all(&tests).expect("create consumer tests directory");
    fs::write(
        tests.join("consumer.rs"),
        r##"use optional_untyped_field::Holder;

#[test]
fn optional_untyped_property_compiles_and_round_trips() {
    let holder: Holder = serde_json::from_str(r#"{"name":"a"}"#).expect("deserialize without payload");
    assert_eq!(holder.payload, serde_json::Value::Null);
    let json = serde_json::to_value(&holder).expect("serialize holder");
    assert!(json.get("payload").is_none(), "absent payload must be omitted: {json}");

    let with: Holder =
        serde_json::from_str(r#"{"name":"a","payload":{"k":1}}"#).expect("deserialize with payload");
    let json = serde_json::to_value(&with).expect("serialize holder with payload");
    assert_eq!(json.get("payload"), Some(&serde_json::json!({"k": 1})));
}
"##,
    )
    .expect("write consumer test");

    let output = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&root)
        .output()
        .expect("spawn cargo test");
    assert_success(output, "generated models compile and round-trip");
    let _ = fs::remove_dir_all(&root);
}
