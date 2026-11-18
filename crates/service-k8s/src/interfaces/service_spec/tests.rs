use super::*;
use crate::domain::profile::{
    AlertRule, Capabilities, Partitioning, ProfileError, ReadScaling, ServiceProfile,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
struct LumenConfig {
    cache_mb: u32,
    mode: String,
}

struct Lumen;

impl ServiceProfile for Lumen {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = LumenConfig;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

struct Empty;

impl ServiceProfile for Empty {
    const KIND: &'static str = "";
    const PREFIX: &'static str = "EMPTY";
    type Config = LumenConfig;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

fn minimal_spec(config: serde_json::Value) -> serde_json::Value {
    json!({
        "image": {"image": "x:1"},
        "storage": {"size": "1Gi"},
        "auth": {"mode": "none"},
        "config": config
    })
}

fn spec_schema(v: &serde_json::Value) -> &serde_json::Value {
    &v["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
}

#[test]
fn crd_has_the_kind_and_group_of_the_profile() {
    let v = crd_for::<Lumen>("lumen.example").unwrap();
    assert_eq!(v["spec"]["group"], "lumen.example");
    assert_eq!(v["spec"]["names"]["kind"], "Lumen");
    assert_eq!(v["metadata"]["name"], "lumens.lumen.example");
    assert_eq!(v["spec"]["scope"], "Namespaced");
}

#[test]
fn crd_schema_holds_every_core_spec_block() {
    let v = crd_for::<Lumen>("lumen.example").unwrap();
    let props = &spec_schema(&v)["properties"];
    for key in [
        "image",
        "resources",
        "storage",
        "replicas",
        "auth",
        "peerTls",
        "backup",
        "placement",
        "monitoring",
        "imagePullSecrets",
        "config",
    ] {
        assert!(props.get(key).is_some(), "spec schema has no `{key}`");
    }
}

#[test]
fn crd_schema_has_no_refs() {
    let v = crd_for::<Lumen>("lumen.example").unwrap();
    let text = v.to_string();
    assert!(!text.contains("\"$ref\""), "CRD has a $ref");
    assert!(!text.contains("\"$schema\""), "CRD has a $schema");
}

#[test]
fn config_schema_is_the_product_config_type_and_is_not_interpreted() {
    let v = crd_for::<Lumen>("lumen.example").unwrap();
    let cfg = &spec_schema(&v)["properties"]["config"];
    assert!(cfg["properties"]["mode"].is_object());
    let cache = &cfg["properties"]["cache_mb"];
    assert!(
        cache.get("format").is_none(),
        "uint32 format must be removed"
    );
    assert_eq!(cache["minimum"].as_f64(), Some(0.0));

    let spec = serde_json::from_value::<ServiceSpec<serde_json::Value>>(minimal_spec(
        json!({"anything": [1, 2]}),
    ))
    .unwrap();
    assert_eq!(spec.config, json!({"anything": [1, 2]}));
}

#[test]
fn unknown_field_in_a_core_block_is_refused() {
    let mut v = minimal_spec(json!({}));
    v["storage"]["bogus"] = json!(1);
    let err = serde_json::from_value::<ServiceSpec<serde_json::Value>>(v).unwrap_err();
    assert!(err.to_string().contains("bogus"), "error was: {err}");

    let mut top = minimal_spec(json!({}));
    top["nope"] = json!(1);
    assert!(serde_json::from_value::<ServiceSpec<serde_json::Value>>(top).is_err());
}

#[test]
fn a_minimal_spec_gets_defaults() {
    let spec =
        serde_json::from_value::<ServiceSpec<serde_json::Value>>(minimal_spec(json!({}))).unwrap();
    assert_eq!(spec.replicas.replicas_per_shard, 1);
    assert_eq!(spec.replicas.voters, 1);
    assert!(!spec.peer_tls.enabled);
    assert!(spec.image_pull_secrets.is_empty());
}

#[test]
fn an_invalid_profile_is_refused() {
    assert_eq!(crd_for::<Empty>("e.example"), Err(ProfileError::EmptyKind));
}
