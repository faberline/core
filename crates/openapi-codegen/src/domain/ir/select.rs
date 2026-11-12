//! Keep only the selected operations of an OpenAPI document, and only the
//! component schemas that those operations reach through `$ref`.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::domain::{OperationSelection, OperationSelectionError};

/// The HTTP methods of a path item, the same set as `METHODS` in
/// `operations.rs`.
const METHODS: &[&str] = &["get", "put", "post", "delete", "patch", "query"];

const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

/// Spec JSON text -> spec JSON text with only the selected operations and the
/// schemas they reach.
pub fn select_operations(
    spec_json: &str,
    selection: &OperationSelection,
) -> Result<String, OperationSelectionError> {
    let mut spec: Value = match serde_json::from_str(spec_json) {
        Ok(value) => value,
        // The emitter reports the parse error as it does today.
        Err(_) => return Ok(spec_json.to_string()),
    };
    let include = selection.include();

    // Keep the selected operations; drop the rest and every path item left
    // without an operation. `seen` records each operationId of the spec.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    if let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) {
        paths.retain(|_, item| match item.as_object_mut() {
            Some(item) => prune_path_item(item, include, &mut seen),
            None => true,
        });
    }

    if let Some(id) = include.iter().find(|id| !seen.contains(*id)) {
        return Err(OperationSelectionError::Unknown(id.clone()));
    }

    // Follow `$ref`s to component schemas, from the kept paths, transitively.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut work: Vec<String> = Vec::new();
    if let Some(paths) = spec.get("paths") {
        collect_schema_refs(paths, &mut work);
    }
    while let Some(key) = work.pop() {
        if !reached.insert(key.clone()) {
            continue;
        }
        let schema = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.get(&key));
        if let Some(schema) = schema {
            collect_schema_refs(schema, &mut work);
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

/// Keep the selected operations of one path item. Returns whether any
/// operation is left.
fn prune_path_item(
    item: &mut Map<String, Value>,
    include: &[String],
    seen: &mut BTreeSet<String>,
) -> bool {
    let mut left = false;
    for method in METHODS {
        let keep = item
            .get(*method)
            .is_some_and(|op| keep_operation(op, include, seen));
        if keep {
            left = true;
        } else {
            item.remove(*method);
        }
    }
    if let Some(extra) = item
        .get_mut("additionalOperations")
        .and_then(Value::as_object_mut)
    {
        extra.retain(|_, op| keep_operation(op, include, seen));
        if extra.is_empty() {
            item.remove("additionalOperations");
        } else {
            left = true;
        }
    }
    left
}

/// Whether an operation is selected. It is never selected without an
/// `operationId`. Every `operationId` it has is recorded in `seen`.
fn keep_operation(op: &Value, include: &[String], seen: &mut BTreeSet<String>) -> bool {
    match op.get("operationId").and_then(Value::as_str) {
        Some(id) => {
            seen.insert(id.to_string());
            include.iter().any(|selected| selected == id)
        }
        None => false,
    }
}

/// Add the key of every `$ref` to a component schema, found anywhere in
/// `value`.
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
    use serde_json::Value;

    use super::select_operations;
    use crate::domain::OperationSelection;

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

    fn select(spec: &str, ids: &[&str]) -> Result<Value, String> {
        let sel = OperationSelection::new(ids.iter().map(|s| s.to_string()).collect()).unwrap();
        let out = select_operations(spec, &sel).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&out).expect("output is JSON"))
    }

    /// The keys of a JSON object, sorted: the order does not matter here.
    fn keys(map: &Value) -> Vec<String> {
        let mut keys: Vec<String> = map
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    #[test]
    fn keeps_only_the_selected_operation_and_the_schemas_it_reaches() {
        let out = select(SPEC, &["getTask"]).unwrap();
        assert_eq!(keys(&out["paths"]), ["/tasks/{id}"]);
        // Direct $ref (Task), nested $ref (Task.labels -> Label) and the
        // error response (ErrorEnvelope) are kept. User and Audit are not.
        assert_eq!(
            keys(&out["components"]["schemas"]),
            ["ErrorEnvelope", "Label", "Task"]
        );
    }

    #[test]
    fn a_schema_cycle_terminates() {
        let spec = r##"{
  "openapi": "3.0.0",
  "paths": {"/a": {"get": {"operationId": "cyc", "responses": {"200": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/A"}}}}}}}},
  "components": {"schemas": {
    "A": {"type": "object", "properties": {"b": {"$ref": "#/components/schemas/B"}}},
    "B": {"type": "object", "properties": {"a": {"$ref": "#/components/schemas/A"}}},
    "Z": {"type": "string"}
  }}
}"##;
        let out = select(spec, &["cyc"]).unwrap();
        assert_eq!(keys(&out["components"]["schemas"]), ["A", "B"]);
    }

    #[test]
    fn unknown_operation_id_names_the_id() {
        let sel = OperationSelection::new(vec!["nope".into()]).unwrap();
        let err = select_operations(SPEC, &sel).unwrap_err();
        assert_eq!(
            err,
            crate::domain::OperationSelectionError::Unknown("nope".into())
        );
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn an_operation_without_an_id_is_dropped() {
        let spec = r##"{
  "openapi": "3.0.0",
  "paths": {"/tasks/{id}": {
    "get": {"operationId": "getTask", "responses": {}},
    "post": {"responses": {}}
  }},
  "components": {"schemas": {}}
}"##;
        let out = select(spec, &["getTask"]).unwrap();
        let item = &out["paths"]["/tasks/{id}"];
        assert_eq!(keys(item), ["get"]);
    }

    #[test]
    fn invalid_json_is_returned_unchanged() {
        let sel = OperationSelection::new(vec!["getTask".into()]).unwrap();
        assert_eq!(select_operations("not json", &sel).unwrap(), "not json");
    }
}
