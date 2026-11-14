use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, OperationSelection, RustTarget,
    TargetProfile,
};

const SUBSET_SPEC: &str = r##"{
  "openapi": "3.0.0",
  "info": { "title": "Subset", "version": "1.0.0" },
  "paths": {
    "/task": { "get": {
      "operationId": "getTask",
      "responses": {
        "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Task" } } } },
        "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
      }
    } },
    "/users": { "get": {
      "operationId": "listUsers",
      "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/User" } } } } } }
    } },
    "/audits": { "get": {
      "operationId": "listAudits",
      "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Audit" } } } } } }
    } }
  },
  "components": { "schemas": {
    "Task": { "type": "object", "properties": { "id": { "type": "string" }, "labels": { "type": "array", "items": { "$ref": "#/components/schemas/Label" } } }, "required": ["id"] },
    "Label": { "type": "string" },
    "ErrorEnvelope": { "type": "object", "properties": { "error": { "type": "string" }, "message": { "type": "string" } } },
    "User": { "type": "object", "properties": { "id": { "type": "string" } } },
    "Audit": { "type": "object", "properties": { "id": { "type": "string" } } }
  } }
}"##;

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-subset-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create subset temp directory");
    path
}

fn materialize(output: &GeneratedOutput, path: &Path) {
    output.write_to_dir(path).expect("write generated output");
    assert!(path.join(".openapi-codegen.json").is_file());
}

fn output(command: &mut Command, what: &str) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("{what}: failed to spawn: {error}"))
}

fn assert_success(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_rust_subset_compiles() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let selection = OperationSelection::new(vec!["getTask".to_string()]).unwrap();
    let opts = GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_hooks(false)
        .with_operations(selection);
    let generated =
        generate_for_target(SUBSET_SPEC, &opts, target).expect("generate Rust subset client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_subset"
version = "0.0.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    )
    .expect("write generated Rust manifest");
    fs::write(
        source.join("main.rs"),
        r#"fn main() {
    let client = generated_subset::client::Client::new("http://127.0.0.1:1");
    let _ = client.get_task();
}
"#,
    )
    .expect("write generated Rust consumer");
    let build = output(
        Command::new("cargo")
            .arg("build")
            .arg("--quiet")
            .arg("--offline")
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", root.join("target")),
        "compile generated Rust subset client",
    );
    assert_success(build, "compile generated Rust subset client");
}
