mod error_display;
mod file_bearer_auth;
mod generation;
mod output;
mod target_profile;

use super::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn full_opts() -> GenOptions {
    GenOptions {
        lang: Lang::Ts,
        target: None,
        spec_path: PathBuf::new(),
        out_dir: PathBuf::new(),
        client_name: "createClient".to_string(),
        http_client: HttpClient::Fetch,
        emit_types: true,
        emit_client: true,
        emit_hooks: true,
    }
}

const MINIMAL: &str = r##"{
      "openapi": "3.0.0",
      "info": { "title": "Mini", "version": "1.0.0" },
      "paths": {
        "/pets": {
          "get": {
            "operationId": "listPets",
            "responses": { "200": { "content": { "application/json": {
              "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Pet" } } } } } }
          }
        }
      },
      "components": { "schemas": {
        "Pet": { "type": "object", "properties": { "id": { "type": "integer" }, "name": { "type": "string" } }, "required": ["id", "name"] }
      } }
    }"##;

const TARGET_PROFILE_SPEC: &str = r##"{
      "openapi": "3.0.0",
      "info": { "title": "Profiles", "version": "1.0.0" },
      "paths": {},
      "components": { "schemas": {
        "Label": { "type": "string" },
        "Pet": { "type": "object", "properties": { "gen": { "type": "string" } }, "required": ["gen"] }
      } }
    }"##;

fn content<'a>(out: &'a GeneratedOutput, name: &str) -> &'a str {
    out.files
        .iter()
        .find(|f| f.rel_path == name)
        .unwrap()
        .contents
        .as_str()
}
