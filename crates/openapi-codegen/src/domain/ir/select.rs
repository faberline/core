//! Keeps only the selected operations of an OpenAPI document, and only the
//! component schemas that those operations reach through `$ref`.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::domain::{OperationSelection, OperationSelectionError};

/// The HTTP methods of a path item, the same set as `METHODS` in `operations.rs`.
const METHODS: &[&str] = &["get", "put", "post", "delete", "patch", "query"];

/// The prefix of a `$ref` to a component schema.
const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

/// Spec JSON text -> spec JSON text with only the selected operations and the
/// schemas they reach.
///
/// Text that is not valid JSON is returned unchanged, so the emitter reports
/// the parse error as it does today.
pub fn select_operations(
    spec_json: &str,
    selection: &OperationSelection,
) -> Result<String, OperationSelectionError> {
    let Ok(mut spec) = serde_json::from_str::<Value>(spec_json) else {
        return Ok(spec_json.to_string());
    };
    let include = selection.include();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    if let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) {
        paths.retain(|_, item| {
            let Some(item) = item.as_object_mut() else {
                return true;
            };
            item.retain(|key, value| {
                if METHODS.contains(&key.as_str()) {
                    return keep_operation(value, include, &mut seen);
                }
                if key == "additionalOperations" {
                    if let Some(extra) = value.as_object_mut() {
                        extra.retain(|_, op| keep_operation(op, include, &mut seen));
                        return !extra.is_empty();
                    }
                }
                true
            });
            item.keys()
                .any(|k| METHODS.contains(&k.as_str()) || k == "additionalOperations")
        });
    }

    if let Some(missing) = include.iter().find(|id| !seen.contains(id.as_str())) {
        return Err(OperationSelectionError::Unknown(missing.clone()));
    }

    let mut pending: Vec<String> = Vec::new();
    if let Some(paths) = spec.get("paths") {
        collect_schema_refs(paths, &mut pending);
    }
    let mut reached: BTreeSet<String> = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !reached.insert(name.clone()) {
            continue;
        }
        let schema = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.get(&name));
        if let Some(schema) = schema {
            collect_schema_refs(schema, &mut pending);
        }
    }

    if let Some(schemas) = spec
        .get_mut("components")
        .and_then(|c| c.get_mut("schemas"))
        .and_then(Value::as_object_mut)
    {
        schemas.retain(|key, _| reached.contains(key));
    }

    Ok(serde_json::to_string(&spec).expect("a JSON value always serializes"))
}

/// Records the `operationId` of one operation. Keeps it (and returns `true`)
/// only when the id is selected; an operation without an id is dropped.
fn keep_operation(op: &Value, include: &[String], seen: &mut BTreeSet<String>) -> bool {
    let Some(id) = op.get("operationId").and_then(Value::as_str) else {
        return false;
    };
    if include.iter().any(|selected| selected == id) {
        seen.insert(id.to_string());
        true
    } else {
        false
    }
}

/// Pushes the schema key of every `$ref` under `value` that points at
/// `#/components/schemas/<key>`.
fn collect_schema_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" {
                    if let Some(name) = child
                        .as_str()
                        .and_then(|r| r.strip_prefix(SCHEMA_REF_PREFIX))
                    {
                        out.push(name.to_string());
                    }
                }
                collect_schema_refs(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_schema_refs(item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::Value;

    use super::*;

    /// Three operations, five schemas. `getTask` reaches `Task`, `Label`
    /// (through `Task.labels`) and `ErrorEnvelope` (the 404 response).
    /// `listUsers` reaches `User`; `listAudits` reaches `Audit`.
    const SPEC: &str = r##"{
      "openapi": "3.0.0",
      "info": {"title": "t", "version": "1"},
      "paths": {
        "/tasks/{id}": {
          "get": {
            "operationId": "getTask",
            "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}],
            "responses": {
              "200": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}},
              "404": {"description": "missing", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorEnvelope"}}}}
            }
          }
        },
        "/users": {
          "get": {
            "operationId": "listUsers",
            "responses": {"200": {"description": "ok", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/User"}}}}}}
          }
        },
        "/audits": {
          "get": {
            "operationId": "listAudits",
            "responses": {"200": {"description": "ok", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Audit"}}}}}}
          }
        }
      },
      "components": {
        "schemas": {
          "Task": {"type": "object", "required": ["id"], "properties": {"id": {"type": "string"}, "labels": {"type": "array", "items": {"$ref": "#/components/schemas/Label"}}}},
          "Label": {"type": "string"},
          "ErrorEnvelope": {"type": "object", "properties": {"error": {"type": "string"}, "message": {"type": "string"}}},
          "User": {"type": "object", "properties": {"id": {"type": "string"}}},
          "Audit": {"type": "object", "properties": {"id": {"type": "string"}}}
        }
      }
    }"##;

    fn select(spec: &str, ids: &[&str]) -> Result<Value, OperationSelectionError> {
        let sel = OperationSelection::new(ids.iter().map(|s| s.to_string()).collect()).unwrap();
        let out = select_operations(spec, &sel)?;
        Ok(serde_json::from_str(&out).expect("output is JSON"))
    }

    fn keys(object: &Value) -> BTreeSet<String> {
        object
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keeps_only_the_selected_operation_and_the_schemas_it_reaches() {
        let out = select(SPEC, &["getTask"]).unwrap();

        assert_eq!(keys(&out["paths"]), set(&["/tasks/{id}"]));
        // Direct $ref (Task), nested $ref through Task.labels (Label) and the
        // error response (ErrorEnvelope). User and Audit are the negative control.
        assert_eq!(
            keys(&out["components"]["schemas"]),
            set(&["ErrorEnvelope", "Label", "Task"])
        );
    }

    #[test]
    fn a_schema_cycle_terminates() {
        let spec = r##"{
          "openapi": "3.0.0",
          "paths": {"/a": {"get": {"operationId": "getA", "responses": {"200": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/A"}}}}}}}},
          "components": {"schemas": {
            "A": {"type": "object", "properties": {"b": {"$ref": "#/components/schemas/B"}}},
            "B": {"type": "object", "properties": {"a": {"$ref": "#/components/schemas/A"}}},
            "Z": {"type": "string"}
          }}
        }"##;

        let out = select(spec, &["getA"]).unwrap();

        assert_eq!(keys(&out["components"]["schemas"]), set(&["A", "B"]));
    }

    #[test]
    fn unknown_operation_id_names_the_id() {
        let err = select(SPEC, &["nope"]).unwrap_err();

        assert_eq!(err, OperationSelectionError::Unknown("nope".into()));
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn an_operation_without_an_id_is_dropped() {
        let spec = r#"{
          "openapi": "3.0.0",
          "paths": {
            "/a": {"get": {"operationId": "getA", "responses": {}}},
            "/noid": {"get": {"responses": {}}}
          }
        }"#;

        let out = select(spec, &["getA"]).unwrap();

        assert_eq!(keys(&out["paths"]), set(&["/a"]));
    }

    #[test]
    fn invalid_json_is_returned_unchanged() {
        let sel = OperationSelection::new(vec!["getTask".into()]).unwrap();

        assert_eq!(
            select_operations("not json", &sel),
            Ok("not json".to_string())
        );
    }
}
