use super::*;
use sha2::{Digest, Sha256};

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

fn sel(ids: &[&str]) -> OperationSelection {
    OperationSelection::new(ids.iter().map(|s| s.to_string()).collect()).unwrap()
}

/// The (path, contents) pairs of a generation result, in emitted order.
fn pairs(output: &GeneratedOutput) -> Vec<(String, String)> {
    output
        .files
        .iter()
        .map(|file| (file.rel_path.clone(), file.contents.clone()))
        .collect()
}

#[test]
fn subset_has_one_method_and_only_reached_schemas() {
    let cases = [
        (
            Lang::Rust,
            "models.rs",
            "client.rs",
            "get_task",
            ["list_users", "list_audits"],
        ),
        (
            Lang::Ts,
            "types.ts",
            "client.ts",
            "getTask",
            ["listUsers", "listAudits"],
        ),
        (
            Lang::Py,
            "models.py",
            "client.py",
            "get_task",
            ["list_users", "list_audits"],
        ),
    ];
    for (lang, models, client, get_name, others) in cases {
        let out = generate(
            SUBSET_SPEC,
            &opts_for(lang).with_operations(sel(&["getTask"])),
        )
        .unwrap();
        let models_text = content(&out, models);
        assert!(models_text.contains("Task"), "{models}: Task missing");
        assert!(
            models_text.contains("ErrorEnvelope"),
            "{models}: ErrorEnvelope missing"
        );
        assert!(
            !models_text.contains("User"),
            "{models}: unreached User emitted"
        );
        assert!(
            !models_text.contains("Audit"),
            "{models}: unreached Audit emitted"
        );
        let client_text = content(&out, client);
        assert!(
            client_text.contains(get_name),
            "{client}: {get_name} missing"
        );
        for other in others {
            assert!(
                !client_text.contains(other),
                "{client}: unselected {other} emitted"
            );
        }
    }
}

#[test]
fn flag_and_toml_give_identical_output() {
    let from_toml = OperationSelection::from_toml("[operations]\ninclude = [\"getTask\"]\n")
        .unwrap()
        .expect("operations table present");
    for lang in [Lang::Ts, Lang::Py, Lang::Rust] {
        let by_flag = generate(
            SUBSET_SPEC,
            &opts_for(lang).with_operations(sel(&["getTask"])),
        )
        .unwrap();
        let by_toml = generate(
            SUBSET_SPEC,
            &opts_for(lang).with_operations(from_toml.clone()),
        )
        .unwrap();
        assert_eq!(pairs(&by_flag), pairs(&by_toml));
    }
}

#[test]
fn unknown_operation_fails_naming_it() {
    let err = generate(
        SUBSET_SPEC,
        &opts_for(Lang::Rust).with_operations(sel(&["nope"])),
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("nope"), "error was: {err:#}");
}

#[test]
fn no_selection_is_unchanged() {
    for lang in [Lang::Ts, Lang::Py, Lang::Rust] {
        let first = generate(SUBSET_SPEC, &opts_for(lang)).unwrap();
        let second = generate(SUBSET_SPEC, &opts_for(lang)).unwrap();
        assert_eq!(pairs(&first), pairs(&second));
        assert_eq!(first.operations, None);
    }
    let out = generate(SUBSET_SPEC, &opts_for(Lang::Rust)).unwrap();
    let models = content(&out, "models.rs");
    assert!(models.contains("User") && models.contains("Audit"));
}

#[test]
fn manifest_records_the_include_list() {
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let plain = generate_for_target(SUBSET_SPEC, &opts_for(Lang::Rust), target).unwrap();
    let manifest = plain.manifest().unwrap();
    assert_eq!(manifest.operations, None);
    assert!(!serde_json::to_string(&manifest)
        .unwrap()
        .contains("\"operations\""));

    let selected = generate_for_target(
        SUBSET_SPEC,
        &opts_for(Lang::Rust).with_operations(sel(&["getTask"])),
        target,
    )
    .unwrap();
    let manifest = selected.manifest().unwrap();
    assert_eq!(manifest.operations, Some(vec!["getTask".to_string()]));
    assert!(serde_json::to_string(&manifest)
        .unwrap()
        .contains("\"operations\":[\"getTask\"]"));
}

#[test]
fn provenance_hash_is_of_the_original_spec() {
    let full = generate(SUBSET_SPEC, &opts_for(Lang::Rust)).unwrap();
    let selected = generate(
        SUBSET_SPEC,
        &opts_for(Lang::Rust).with_operations(sel(&["getTask"])),
    )
    .unwrap();
    let expected = format!("{:x}", Sha256::digest(SUBSET_SPEC.as_bytes()));
    assert_eq!(full.openapi_sha256(), Some(expected.as_str()));
    assert_eq!(selected.openapi_sha256(), Some(expected.as_str()));
}
