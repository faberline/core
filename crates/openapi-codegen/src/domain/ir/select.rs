//! Operation selection on the spec text: keep only the chosen operations and
//! the component schemas that they reach through `$ref`.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::domain::{OperationSelection, OperationSelectionError};

/// The same HTTP method keys as `METHODS` in [`super::operations`], plus
/// `additionalOperations` entries handled separately.
const METHODS: &[&str] = &["get", "put", "post", "delete", "patch", "query"];

const SCHEMA_PREFIX: &str = "#/components/schemas/";

/// Spec JSON text -> spec JSON text with only the selected operations and the
/// schemas they reach. Invalid JSON is returned unchanged so the emitter
/// reports the parse error as it does today.
// Part 2 of #128 (issue #133) calls this from `application/generate.rs`.
#[allow(dead_code)]
pub fn select_operations(
    spec_json: &str,
    selection: &OperationSelection,
) -> Result<String, OperationSelectionError> {
    let Ok(mut spec) = serde_json::from_str::<Value>(spec_json) else {
        return Ok(spec_json.to_string());
    };

    // Every operationId in the spec, selected or not, so that an unknown id
    // is told apart from a known id that was merely not selected.
    let mut seen = BTreeSet::new();
    if let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) {
        paths.retain(|_, item| retain_path_item(item, selection.include(), &mut seen));
    }
    for id in selection.include() {
        if !seen.contains(id) {
            return Err(OperationSelectionError::Unknown(id.clone()));
        }
    }

    let reached = reached_schemas(&spec);
    if let Some(schemas) = spec
        .get_mut("components")
        .and_then(|components| components.get_mut("schemas"))
        .and_then(Value::as_object_mut)
    {
        schemas.retain(|key, _| reached.contains(key));
    }

    Ok(serde_json::to_string(&spec)
        .expect("a parsed JSON value always serializes back to JSON text"))
}

/// Drop the unselected operations of one path item. Returns `false` when no
/// operation is left, so the whole path item is removed.
fn retain_path_item(item: &mut Value, include: &[String], seen: &mut BTreeSet<String>) -> bool {
    let Some(item) = item.as_object_mut() else {
        return true;
    };
    for method in METHODS {
        let keep = match item.get(*method) {
            Some(operation) => keep_operation(operation, include, seen),
            None => continue,
        };
        if !keep {
            item.remove(*method);
        }
    }
    if let Some(extra) = item
        .get_mut("additionalOperations")
        .and_then(Value::as_object_mut)
    {
        extra.retain(|_, operation| keep_operation(operation, include, seen));
        if extra.is_empty() {
            item.remove("additionalOperations");
        }
    }
    METHODS.iter().any(|method| item.contains_key(*method))
        || item.contains_key("additionalOperations")
}

/// Record the operation's id and say whether it is selected. An operation
/// without an `operationId` is never selected.
fn keep_operation(operation: &Value, include: &[String], seen: &mut BTreeSet<String>) -> bool {
    match operation.get("operationId").and_then(Value::as_str) {
        Some(id) => {
            seen.insert(id.to_string());
            include.iter().any(|selected| selected == id)
        }
        None => false,
    }
}

/// Component schema keys reached from the kept paths, transitively through
/// `$ref`. The work list and the result are sets, so a cycle ends.
fn reached_schemas(spec: &Value) -> BTreeSet<String> {
    let mut reached = BTreeSet::new();
    let mut work = Vec::new();
    if let Some(paths) = spec.get("paths") {
        collect_schema_refs(paths, &mut work);
    }
    while let Some(key) = work.pop() {
        if !reached.insert(key.clone()) {
            continue;
        }
        let schema = spec
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(|schemas| schemas.get(&key));
        if let Some(schema) = schema {
            collect_schema_refs(schema, &mut work);
        }
    }
    reached
}

/// Push the schema key of every `#/components/schemas/<key>` `$ref` below
/// `value`. Other `$ref` forms are left alone.
fn collect_schema_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" {
                    if let Some(name) = child.as_str().and_then(|r| r.strip_prefix(SCHEMA_PREFIX)) {
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
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;

    /// Three operations, five schemas. `Task` reaches `Label` (nested) and
    /// `ErrorEnvelope` (error response). `User` and `Audit` are reached only
    /// by the operations that the tests do not select.
    const SPEC: &str = r##"{
  "openapi": "3.0.0",
  "info": { "title": "Tasks", "version": "1.0.0" },
  "paths": {
    "/tasks/{id}": {
      "get": {
        "operationId": "getTask",
        "parameters": [ { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } } ],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Task" } } } },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
        }
      }
    },
    "/users": {
      "get": {
        "operationId": "listUsers",
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/User" } } } } }
        }
      }
    },
    "/audits": {
      "get": {
        "operationId": "listAudits",
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Audit" } } } } }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "Task": { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" }, "labels": { "type": "array", "items": { "$ref": "#/components/schemas/Label" } } } },
      "Label": { "type": "string" },
      "ErrorEnvelope": { "type": "object", "properties": { "error": { "type": "string" }, "message": { "type": "string" } } },
      "User": { "type": "object", "properties": { "id": { "type": "string" } } },
      "Audit": { "type": "object", "properties": { "id": { "type": "string" } } }
    }
  }
}"##;

    fn select(spec: &str, ids: &[&str]) -> Result<String, OperationSelectionError> {
        let selection =
            OperationSelection::new(ids.iter().map(|id| id.to_string()).collect()).unwrap();
        select_operations(spec, &selection)
    }

    /// Schema keys in sorted order, so the assertions compare sets.
    fn schema_keys(out: &Value) -> Vec<String> {
        let mut keys: Vec<String> = out["components"]["schemas"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    #[test]
    fn keeps_only_the_selected_operation_and_the_schemas_it_reaches() {
        let out: Value = serde_json::from_str(&select(SPEC, &["getTask"]).unwrap()).unwrap();
        let paths: Vec<&String> = out["paths"].as_object().unwrap().keys().collect();
        assert_eq!(paths, ["/tasks/{id}"]);
        // Direct $ref (Task), nested $ref through Task.labels (Label), and the
        // error response (ErrorEnvelope). User and Audit must be pruned.
        assert_eq!(schema_keys(&out), ["ErrorEnvelope", "Label", "Task"]);
    }

    #[test]
    fn a_schema_cycle_terminates() {
        let spec = r##"{
  "openapi": "3.0.0",
  "info": { "title": "Cycle", "version": "1.0.0" },
  "paths": { "/a": { "get": { "operationId": "getA", "responses": {
    "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/A" } } } } } } } },
  "components": { "schemas": {
    "A": { "type": "object", "properties": { "b": { "$ref": "#/components/schemas/B" } } },
    "B": { "type": "object", "properties": { "a": { "$ref": "#/components/schemas/A" } } },
    "C": { "type": "string" } } }
}"##;
        let out: Value = serde_json::from_str(&select(spec, &["getA"]).unwrap()).unwrap();
        let keys: BTreeSet<String> = schema_keys(&out).into_iter().collect();
        assert_eq!(keys, BTreeSet::from(["A".to_string(), "B".to_string()]));
    }

    #[test]
    fn unknown_operation_id_names_the_id() {
        let err = select(SPEC, &["nope"]).unwrap_err();
        assert_eq!(err, OperationSelectionError::Unknown("nope".into()));
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn an_operation_without_an_id_is_dropped() {
        let spec = r##"{
  "openapi": "3.0.0",
  "info": { "title": "NoId", "version": "1.0.0" },
  "paths": {
    "/x": { "get": { "responses": { "200": { "description": "ok" } } } },
    "/y": { "get": { "operationId": "getY", "responses": { "200": { "description": "ok" } } } }
  }
}"##;
        let out: Value = serde_json::from_str(&select(spec, &["getY"]).unwrap()).unwrap();
        let paths: Vec<&String> = out["paths"].as_object().unwrap().keys().collect();
        assert_eq!(paths, ["/y"]);
    }

    #[test]
    fn invalid_json_is_returned_unchanged() {
        assert_eq!(select("not json", &["getTask"]).unwrap(), "not json");
    }
}
