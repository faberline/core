use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.0.3",
  "info": { "title": "Pets", "version": "1.0.0" },
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "parameters": [{ "name": "limit", "in": "query", "required": false, "schema": { "type": "integer" } }],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Pet" } } } } }
        }
      },
      "post": {
        "operationId": "createPet",
        "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
        "responses": {
          "201": { "description": "created", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
        }
      }
    },
    "/pets/{petId}": {
      "get": {
        "operationId": "getPetById",
        "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
        }
      },
      "delete": {
        "operationId": "deletePet",
        "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
        "responses": { "204": { "description": "deleted" } }
      }
    }
  },
  "components": {
    "schemas": {
      "Pet": { "type": "object", "properties": { "id": { "type": "integer" }, "name": { "type": "string" } }, "required": ["id", "name"] },
      "ErrorEnvelope": { "type": "object", "properties": { "error": { "type": "string" }, "message": { "type": "string" } }, "required": ["error", "message"] }
    }
  }
}"##;

/// Writes the manifest of a generated server crate named `name` at `root`.
/// The generated code needs `axum` and `service-http`, so the crate copies the
/// repository lock file and builds offline from the warm cargo cache.
pub(crate) fn write_manifest(root: &Path, name: &str) {
    let service_http = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../service-http")
        .canonicalize()
        .expect("canonicalize service-http path");
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2021"

[dependencies]
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
axum = "0.8"
tokio = {{ version = "1", features = ["rt"] }}
service-http = {{ path = "{}" }}
tower = {{ version = "0.5", features = ["util"] }}

[workspace]
"#,
        service_http.display()
    );
    fs::write(root.join("Cargo.toml"), manifest).expect("write generated Rust manifest");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"),
        root.join("Cargo.lock"),
    )
    .expect("copy repository Cargo.lock");
}

const MOCK_ALL: &str = r#"use generated_server_handler::models::Pet;
use generated_server_handler::server::*;

struct Mock;

impl CreateClientHandler for Mock {
    async fn list_pets(&self, _limit: Option<i64>) -> Result<Vec<Pet>, ListPetsError> { Ok(Vec::new()) }
    async fn create_pet(&self, body: Pet) -> Result<Pet, CreatePetError> { Ok(body) }
    async fn get_pet_by_id(&self, _pet_id: i64) -> Result<Pet, GetPetByIdError> { Err(GetPetByIdError::Other(service_http::ApiErr::new(axum::http::StatusCode::NOT_FOUND, "not_found", "no pet"))) }
    async fn delete_pet(&self, _pet_id: i64) -> Result<(), DeletePetError> { Ok(()) }
}

fn assert_handler<H: CreateClientHandler>(_: &H) {}

fn main() { assert_handler(&Mock); }
"#;

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-server-handler-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create server handler temp directory");
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

/// Generates the server module into a fresh crate at `root` and writes `main_rs`.
fn generated_crate(root: &Path, main_rs: &str) {
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let opts = GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_server(true)
        .with_emit_client(false);
    let generated =
        generate_for_target(SPEC, &opts, target).expect("generate Rust server handler trait");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    write_manifest(root, "generated_server_handler");
    fs::write(source.join("main.rs"), main_rs).expect("write generated Rust consumer");
}

fn build(root: &Path) -> Output {
    Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo build for generated server handler")
}

#[test]
fn generated_server_trait_compiles() {
    let root = temp_dir("compiles");
    generated_crate(&root, MOCK_ALL);
    let build = build(&root);
    assert_success(build, "compile generated Rust server handler trait");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn generated_server_trait_rejects_a_missing_method() {
    // Negative control: `Mock` leaves out `delete_pet`, so the build must fail.
    let root = temp_dir("missing");
    let without_delete = MOCK_ALL.replace(
        "    async fn delete_pet(&self, _pet_id: i64) -> Result<(), DeletePetError> { Ok(()) }\n",
        "",
    );
    assert_ne!(without_delete, MOCK_ALL, "fixture edit must apply");
    generated_crate(&root, &without_delete);
    let output = build(&root);
    assert!(
        !output.status.success(),
        "a handler that omits delete_pet must not compile"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("delete_pet"), "stderr:\n{stderr}");
    let _ = fs::remove_dir_all(&root);
}
