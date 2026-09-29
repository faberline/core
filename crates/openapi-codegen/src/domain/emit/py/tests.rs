mod compile;
mod h2c_connection;
mod h2c_server;
mod h2c_smoke;
mod package;
mod runtime;
mod tls_alpn;

use super::*;
use crate::{HttpClient, Lang};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use h2c_server::{
    spawn_h2c_bidi_server, spawn_h2c_multiplex_server, spawn_h2c_sequential_server,
    spawn_h2c_smoke_server,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const SPEC: &str = r##"{
      "openapi": "3.0.0",
      "info": { "title": "Mini", "version": "1.0.0" },
      "paths": {
        "/pets/{petId}": {
          "get": {
            "operationId": "getPetById",
            "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
            "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          }
        }
      },
      "components": { "schemas": {
        "Pet": { "type": "object", "properties": { "id": { "type": "integer" }, "name": { "type": "string" }, "tag": { "type": "string" } }, "required": ["id", "name"] }
      } }
    }"##;

const RECURSIVE_UNION_SPEC: &str = r##"{
      "openapi": "3.0.0",
      "info": { "title": "Mini", "version": "1.0.0" },
      "paths": {},
      "components": { "schemas": {
        "MatchQuery": {
          "type": "object",
          "properties": {
            "field": { "type": "string" },
            "text": { "type": "string" }
          },
          "required": ["field", "text"]
        },
        "TermQuery": {
          "type": "object",
          "properties": {
            "field": { "type": "string" },
            "value": { "type": "string" }
          },
          "required": ["field", "value"]
        },
        "QueryNode": {
          "oneOf": [
            {
              "type": "object",
              "required": ["match"],
              "properties": { "match": { "$ref": "#/components/schemas/MatchQuery" } }
            },
            {
              "type": "object",
              "required": ["term"],
              "properties": { "term": { "$ref": "#/components/schemas/TermQuery" } }
            },
            {
              "type": "object",
              "required": ["and"],
              "properties": {
                "and": {
                  "type": "array",
                  "items": { "$ref": "#/components/schemas/QueryNode" }
                }
              }
            },
            {
              "type": "object",
              "required": ["not"],
              "properties": { "not": { "$ref": "#/components/schemas/QueryNode" } }
            }
          ]
        },
        "SearchRequest": {
          "type": "object",
          "properties": { "query": { "$ref": "#/components/schemas/QueryNode" } },
          "required": ["query"]
        }
      } }
    }"##;

/// OpenAPI 3.2 fixture: a `query` operation (RFC 10008 HTTP QUERY) with a
/// sibling `post` twin on the same path — the default POST-twin fallback
/// convention (epic #1296).
const SPEC_32_QUERY: &str = r##"{
      "openapi": "3.2.0",
      "info": { "title": "Mini", "version": "1.0.0" },
      "paths": {
        "/pets": {
          "query": {
            "operationId": "searchPets",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object" } } } },
            "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          },
          "post": {
            "operationId": "createPet",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
            "responses": { "201": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          }
        }
      },
      "components": { "schemas": {
        "Pet": { "type": "object", "properties": { "id": { "type": "integer" }, "name": { "type": "string" } }, "required": ["id", "name"] }
      } }
    }"##;

fn opts() -> GenOptions {
    GenOptions {
        lang: Lang::Py,
        target: None,
        spec_path: PathBuf::new(),
        out_dir: PathBuf::new(),
        client_name: "Client".to_string(),
        http_client: HttpClient::Fetch,
        emit_types: true,
        emit_client: true,
        emit_hooks: false,
    }
}

fn file<'a>(out: &'a GeneratedOutput, name: &str) -> &'a str {
    out.files
        .iter()
        .find(|f| f.rel_path == name)
        .unwrap()
        .contents
        .as_str()
}

fn unique_temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "openapi-codegen-py-{}-{nonce}-{serial}",
        std::process::id()
    ))
}

fn write_generated_python_package(out: &GeneratedOutput) -> PathBuf {
    let dir = unique_temp_dir();
    let pkg = dir.join("generated_api");
    fs::create_dir_all(&pkg).unwrap();
    for generated in &out.files {
        fs::write(pkg.join(&generated.rel_path), &generated.contents).unwrap();
    }
    dir
}
