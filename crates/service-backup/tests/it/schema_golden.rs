//! Golden pins for the `schemars` JSON schema of every backup CRD wire type.
//! Downstream CRDs (lumen, tape, relay, defer, keep) embed these schemas.
//!
//! schemars 1 writes JSON Schema 2020-12 (`$defs`, no `definitions`), so the
//! schema text is no longer byte-equal to the draft-07 text. These tests pin the
//! shape that the CRDs depend on instead: the `required` list, the property
//! names and the property types of every object.

use serde_json::{json, Value};
use service_backup::{BackupDestination, BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};

/// Shape of one object schema: sorted `required` plus, per property, its type.
/// A `$ref` (or an `allOf` with one `$ref`) is reported as `ref:<Name>`.
fn object_shape(node: &Value) -> Value {
    let mut required: Vec<String> = node["required"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
        .unwrap_or_default();
    required.sort();

    let mut properties = serde_json::Map::new();
    if let Some(props) = node["properties"].as_object() {
        for (name, prop) in props {
            properties.insert(name.clone(), property_type(prop));
        }
    }
    json!({ "required": required, "properties": properties })
}

fn property_type(prop: &Value) -> Value {
    if let Some(r) = prop["$ref"].as_str() {
        return json!(format!("ref:{}", r.rsplit('/').next().unwrap()));
    }
    if let Some(r) = prop["allOf"][0]["$ref"].as_str() {
        return json!(format!("ref:{}", r.rsplit('/').next().unwrap()));
    }
    match &prop["type"] {
        Value::Array(types) => {
            let mut names: Vec<String> = types
                .iter()
                .map(|t| t.as_str().unwrap().to_string())
                .collect();
            names.sort();
            json!(names)
        }
        other => other.clone(),
    }
}

fn shape<T: schemars::JsonSchema>() -> Value {
    let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    match schema.get("oneOf") {
        Some(variants) => Value::Array(
            variants
                .as_array()
                .unwrap()
                .iter()
                .map(object_shape)
                .collect(),
        ),
        None => object_shape(&schema),
    }
}

#[test]
fn backup_destination_schema_is_pinned() {
    assert_eq!(
        shape::<BackupDestination>(),
        json!([
            {
                "required": ["path", "type"],
                "properties": { "path": "string", "prefix": ["null", "string"], "type": "string" }
            },
            {
                "required": ["bucket", "type"],
                "properties": {
                    "bucket": "string",
                    "credentials_secret": ["null", "string"],
                    "endpoint": ["null", "string"],
                    "prefix": "string",
                    "region": ["null", "string"],
                    "type": "string"
                }
            },
            {
                "required": ["bucket", "type"],
                "properties": {
                    "bucket": "string",
                    "credentials_secret": ["null", "string"],
                    "prefix": "string",
                    "type": "string"
                }
            }
        ])
    );
}

#[test]
fn scheduled_backup_policy_schema_is_pinned() {
    assert_eq!(
        shape::<ScheduledBackupPolicy>(),
        json!({
            "required": ["destination", "schedule"],
            "properties": {
                "destination": "string",
                "retentionSecs": ["integer", "null"],
                "schedule": "string"
            }
        })
    );
}

#[test]
fn backup_policy_schema_is_pinned() {
    assert_eq!(
        shape::<BackupPolicy>(),
        json!({
            "required": ["destination", "schedule"],
            "properties": {
                "destination": "ref:BackupDestination",
                "retention": "ref:RetentionPolicy",
                "schedule": "string"
            }
        })
    );
}

#[test]
fn retention_policy_schema_is_pinned() {
    assert_eq!(
        shape::<RetentionPolicy>(),
        json!({
            "required": [],
            "properties": { "maxAgeSeconds": ["integer", "null"] }
        })
    );
}

#[test]
fn scheduled_backup_policy_is_a_schemars_1_schema() {
    let schema = serde_json::to_value(schemars::schema_for!(ScheduledBackupPolicy)).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}
