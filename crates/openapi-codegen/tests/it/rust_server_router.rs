use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

use crate::rust_server_handler::write_manifest;

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

/// `SPEC` without `deletePet`: the router then has no `DELETE /pets/{petId}`.
const SPEC_WITHOUT_DELETE: &str = r##"{
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

/// The consumer crate: imports, the readiness hook and the `call` helper.
const MAIN_HEAD: &str = r#"use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use generated_server_router::models::Pet;
use generated_server_router::server::*;
use std::sync::Arc;
use tower::ServiceExt;

struct Ready;
impl service_http::ReadinessHook for Ready {
    fn is_draining(&self) -> bool { false }
}

fn spec_json() -> String { "{}".to_string() }

async fn call(method: Method, uri: &str, body: Option<&str>) -> (StatusCode, String) {
    let app = router(Arc::new(Mock), Arc::new(Ready), spec_json);
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(text) => {
            request = request.header("content-type", "application/json");
            Body::from(text.to_string())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(request.body(body).expect("request")).await.expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

"#;

/// The `Mock` with every operation of `SPEC`.
const MOCK_FULL: &str = r#"struct Mock;

impl CreateClientHandler for Mock {
    async fn list_pets(&self, limit: Option<i64>) -> Result<Vec<Pet>, ListPetsError> {
        Ok(vec![Pet { id: limit.unwrap_or(0), name: "rex".to_string() }])
    }
    async fn create_pet(&self, body: Pet) -> Result<Pet, CreatePetError> { Ok(body) }
    async fn get_pet_by_id(&self, pet_id: i64) -> Result<Pet, GetPetByIdError> {
        if pet_id == 404 {
            Err(GetPetByIdError::Status404(service_http::ErrorEnvelope {
                error: "not_found".to_string(),
                message: "no pet".to_string(),
            }))
        } else {
            Ok(Pet { id: pet_id, name: "rex".to_string() })
        }
    }
    async fn delete_pet(&self, _pet_id: i64) -> Result<(), DeletePetError> { Ok(()) }
}

"#;

/// The `Mock` of `SPEC_WITHOUT_DELETE`: no `delete_pet`.
const MOCK_WITHOUT_DELETE: &str = r#"struct Mock;

impl CreateClientHandler for Mock {
    async fn list_pets(&self, limit: Option<i64>) -> Result<Vec<Pet>, ListPetsError> {
        Ok(vec![Pet { id: limit.unwrap_or(0), name: "rex".to_string() }])
    }
    async fn create_pet(&self, body: Pet) -> Result<Pet, CreatePetError> { Ok(body) }
    async fn get_pet_by_id(&self, pet_id: i64) -> Result<Pet, GetPetByIdError> {
        if pet_id == 404 {
            Err(GetPetByIdError::Status404(service_http::ErrorEnvelope {
                error: "not_found".to_string(),
                message: "no pet".to_string(),
            }))
        } else {
            Ok(Pet { id: pet_id, name: "rex".to_string() })
        }
    }
}

"#;

/// The `main` that calls every route and checks status and body.
const MAIN_TAIL: &str = r##"fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
    rt.block_on(async {
        let (s, b) = call(Method::GET, "/pets?limit=7", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, r#"[{"id":7,"name":"rex"}]"#);
        let (s, b) = call(Method::POST, "/pets", Some(r#"{"id":1,"name":"tom"}"#)).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, r#"{"id":1,"name":"tom"}"#);
        let (s, b) = call(Method::GET, "/pets/5", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, r#"{"id":5,"name":"rex"}"#);
        let (s, b) = call(Method::GET, "/pets/404", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(b, r#"{"error":"not_found","message":"no pet"}"#);
        let (s, _) = call(Method::DELETE, "/pets/5", None).await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (s, _) = call(Method::GET, "/healthz", None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = call(Method::GET, "/readyz", None).await;
        assert_eq!(s, StatusCode::OK);
    });
}
"##;

fn main_source(mock: &str) -> String {
    [MAIN_HEAD, mock, MAIN_TAIL].concat()
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-server-router-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create server router temp directory");
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

/// Generates the server module from `spec` into a fresh crate, then runs
/// `main_rs` against it. Returns the output of `cargo run`.
fn build_and_run(label: &str, spec: &str, main_rs: &str) -> Output {
    let root = temp_dir(label);
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let opts = GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_server(true)
        .with_emit_client(false);
    let generated = generate_for_target(spec, &opts, target).expect("generate Rust server router");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    write_manifest(&root, "generated_server_router");
    fs::write(source.join("main.rs"), main_rs).expect("write generated Rust consumer");
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo run for generated server router");
    let _ = fs::remove_dir_all(&root);
    output
}

#[test]
fn generated_router_serves_every_route() {
    let output = build_and_run("serves", SPEC, &main_source(MOCK_FULL));
    assert_success(output, "generated router serves every route");
}

#[test]
fn generated_router_without_a_route_fails_its_test() {
    // Negative control: the spec has no DELETE route, so the router answers
    // 405 to `DELETE /pets/5` and the `main` assertion must fail at run time.
    let output = build_and_run(
        "missing",
        SPEC_WITHOUT_DELETE,
        &main_source(MOCK_WITHOUT_DELETE),
    );
    assert!(
        !output.status.success(),
        "a router without DELETE /pets/{{petId}} must fail the consumer run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("assertion"), "stderr:\n{stderr}");
}
