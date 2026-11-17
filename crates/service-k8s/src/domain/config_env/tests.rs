use super::*;
use crate::domain::profile::{AlertRule, Capabilities, Partitioning, ReadScaling, ServiceProfile};
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Db {
    host: String,
    port: u16,
    password: SecretRef,
}

#[derive(Serialize)]
struct LumenConfig {
    cache_mb: u32,
    enabled: bool,
    db: Db,
    tags: Vec<String>,
    label: Option<String>,
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

struct Raw;

impl ServiceProfile for Raw {
    const KIND: &'static str = "Raw";
    const PREFIX: &'static str = "RAW";
    type Config = serde_json::Value;

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

fn sample() -> LumenConfig {
    LumenConfig {
        cache_mb: 64,
        enabled: true,
        db: Db {
            host: "h".into(),
            port: 5432,
            password: SecretRef {
                secret_name: "lumen-db".into(),
                key: "pw".into(),
            },
        },
        tags: vec!["a".into(), "b".into()],
        label: None,
    }
}

fn pairs(list: &[(&str, &str)]) -> BTreeMap<String, String> {
    list.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn every_config_key_reaches_the_config_map_data() {
    let env = config_env::<Lumen>(&sample()).expect("sample config flattens");

    let expected = pairs(&[
        ("LUMEN_CACHE_MB", "64"),
        ("LUMEN_ENABLED", "true"),
        ("LUMEN_DB_HOST", "h"),
        ("LUMEN_DB_PORT", "5432"),
        ("LUMEN_TAGS", "[\"a\",\"b\"]"),
        (
            "LUMEN_DB_PASSWORD",
            "/etc/core/secrets/lumen-db-password/pw",
        ),
    ]);
    assert_eq!(env.data, expected);
    // `label` is None, so it has no key at all.
    assert!(!env.data.contains_key("LUMEN_LABEL"));

    let map = config_map("lumen-config", "ns", &env);
    assert_eq!(map["metadata"]["name"], "lumen-config");
    assert_eq!(map["data"].as_object().expect("data is an object").len(), 6);
}

#[test]
fn camel_case_and_snake_case_keys_flatten_the_same() {
    let env = config_env::<Raw>(&json!({"cacheMb": 1, "inner": {"maxConns": 2}}))
        .expect("config flattens");

    assert_eq!(
        env.data,
        pairs(&[("RAW_CACHE_MB", "1"), ("RAW_INNER_MAX_CONNS", "2")])
    );
}

#[test]
fn a_secret_ref_is_mounted_as_a_file_and_not_put_in_the_config_map() {
    let env = config_env::<Lumen>(&sample()).expect("sample config flattens");

    assert_eq!(env.secret_mounts.len(), 1);
    let mount = &env.secret_mounts[0];
    assert_eq!(
        *mount,
        SecretMount {
            env_key: "LUMEN_DB_PASSWORD".into(),
            secret_name: "lumen-db".into(),
            secret_key: "pw".into(),
            volume_name: "secret-lumen-db-password".into(),
            mount_path: "/etc/core/secrets/lumen-db-password".into(),
            file_path: "/etc/core/secrets/lumen-db-password/pw".into(),
        }
    );
    assert_eq!(env.data["LUMEN_DB_PASSWORD"], mount.file_path);

    // The Secret name and the reference object never reach the ConfigMap data.
    for value in env.data.values() {
        assert!(!value.contains("secretName"));
        assert_ne!(value, "lumen-db");
    }

    assert_eq!(
        secret_volumes(&env),
        vec![json!({
            "name": "secret-lumen-db-password",
            "secret": {
                "secretName": "lumen-db",
                "defaultMode": 292,
                "items": [{"key": "pw", "path": "pw"}]
            }
        })]
    );
    assert_eq!(
        secret_volume_mounts(&env),
        vec![json!({
            "name": "secret-lumen-db-password",
            "mountPath": "/etc/core/secrets/lumen-db-password",
            "readOnly": true
        })]
    );
}

#[test]
fn a_config_without_a_secret_ref_has_no_mounts() {
    let env = config_env::<Raw>(&json!({"a": 1})).expect("config flattens");

    assert!(env.secret_mounts.is_empty());
    assert!(secret_volumes(&env).is_empty());
    assert!(secret_volume_mounts(&env).is_empty());
}

#[test]
fn env_from_names_the_config_map() {
    assert_eq!(env_from("c"), json!({"configMapRef": {"name": "c"}}));
}

#[test]
fn a_secret_ref_in_a_list_is_refused() {
    let err = config_env::<Raw>(&json!({"xs": [{"secretName": "s", "key": "k"}]}))
        .expect_err("a secret ref in a list is refused");

    assert_eq!(
        err,
        ConfigEnvError::SecretRefInList {
            path: "xs".to_string()
        }
    );
}

#[test]
fn a_config_that_is_not_an_object_is_refused() {
    let err = config_env::<Raw>(&json!(5)).expect_err("a number is refused");

    assert_eq!(err, ConfigEnvError::NotAnObject);
}

#[test]
fn an_invalid_key_is_refused() {
    let err = config_env::<Raw>(&json!({"a-b": 1})).expect_err("a dash is refused");

    assert_eq!(
        err,
        ConfigEnvError::InvalidKey {
            path: "a-b".to_string()
        }
    );
}

#[test]
fn two_fields_that_flatten_to_one_key_are_refused() {
    let err = config_env::<Raw>(&json!({"a_b": 1, "a": {"b": 2}}))
        .expect_err("two fields to one key are refused");

    assert_eq!(
        err,
        ConfigEnvError::DuplicateKey {
            key: "RAW_A_B".to_string()
        }
    );
}
