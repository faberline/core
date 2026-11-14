use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

use super::rust_server_handler::write_manifest;

const SPEC: &str = r##"{
  "openapi": "3.0.3",
  "info": { "title": "Things", "version": "1.0.0" },
  "paths": {
    "/things/{id}": {
      "get": {
        "operationId": "getThing",
        "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Thing" } } } },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } },
          "4XX": { "description": "client", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } },
          "default": { "description": "other", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "Thing": { "type": "object", "properties": { "id": { "type": "integer" } }, "required": ["id"] },
      "ErrorEnvelope": { "type": "object", "properties": { "error": { "type": "string" }, "message": { "type": "string" } }, "required": ["error", "message"] }
    }
  }
}"##;

const MAIN_RS: &str = r#"use axum::http::StatusCode;
use axum::response::IntoResponse;
use generated_server_errors::server::GetThingError;
use service_http::{ApiErr, ErrorEnvelope};

fn render(e: GetThingError) -> (StatusCode, serde_json::Value) {
    let response = e.into_response();
    let status = response.status();
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    let bytes = rt
        .block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json"))
}

fn env(error: &str, message: &str) -> ErrorEnvelope {
    ErrorEnvelope { error: error.to_string(), message: message.to_string() }
}

fn main() {
    let (s, b) = render(GetThingError::Status404(env("not_found", "no thing")));
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(b, serde_json::json!({"error": "not_found", "message": "no thing"}));
    let (s, b) = render(GetThingError::Status4XX(StatusCode::CONFLICT, env("conflict", "busy")));
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(b, serde_json::json!({"error": "conflict", "message": "busy"}));
    let (s, b) = render(GetThingError::Default(StatusCode::BAD_GATEWAY, env("upstream", "down")));
    assert_eq!(s, StatusCode::BAD_GATEWAY);
    assert_eq!(b["error"], "upstream");
    let (s, b) = render(GetThingError::from(ApiErr::new(StatusCode::FORBIDDEN, "forbidden", "no")));
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(b, serde_json::json!({"error": "forbidden", "message": "no"}));
}
"#;

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-server-errors-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create server errors temp directory");
    path
}

fn materialize(output: &GeneratedOutput, path: &Path) {
    output.write_to_dir(path).expect("write generated output");
    assert!(path.join(".openapi-codegen.json").is_file());
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

/// Generates the server module into a fresh crate, writes `main_rs`, and runs it.
fn build_and_run(label: &str, main_rs: &str) -> Output {
    let root = temp_dir(label);
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let opts = GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_server(true)
        .with_emit_client(false);
    let generated =
        generate_for_target(SPEC, &opts, target).expect("generate Rust server error types");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    write_manifest(&root, "generated_server_errors");
    fs::write(source.join("main.rs"), main_rs).expect("write generated Rust consumer");
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo run for generated server errors");
    let _ = fs::remove_dir_all(&root);
    output
}

#[test]
fn generated_error_types_render_declared_status_and_json_body() {
    let output = build_and_run("renders", MAIN_RS);
    assert_success(output, "render generated server error types");
}

#[test]
fn wrong_expected_status_fails_the_generated_check() {
    // Negative control: the first expectation is wrong (the declared 404 is not 200).
    let wrong = MAIN_RS.replace(
        "assert_eq!(s, StatusCode::NOT_FOUND);",
        "assert_eq!(s, StatusCode::OK);",
    );
    assert_ne!(wrong, MAIN_RS, "fixture edit must apply");
    let output = build_and_run("wrong", &wrong);
    assert!(
        !output.status.success(),
        "a wrong expected status must fail the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("assertion"), "stderr:\n{stderr}");
}
