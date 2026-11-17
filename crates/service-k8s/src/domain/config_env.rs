//! Flattens a service's config into `<PREFIX>_*` environment keys (ADR 0002 D5).
//!
//! Plain values go into the ConfigMap data. A [`SecretRef`] never goes into the
//! ConfigMap: it becomes a Secret volume mounted as a file, and the `<PREFIX>_*`
//! key holds the path of that file. Core never reads a Secret value.

use crate::domain::profile::ServiceProfile;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Directory under which every secret reference is mounted.
pub const SECRET_MOUNT_ROOT: &str = "/etc/core/secrets";

/// `0444`: readable by any UID the pod runs as, writable by none.
const READ_ONLY_MODE: u32 = 0o444;

/// A reference to one key of one Secret, as it appears in a config.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretRef {
    /// Name of the Secret.
    pub secret_name: String,
    /// Key inside that Secret.
    pub key: String,
}

/// One secret file that a pod mounts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretMount {
    /// The `<PREFIX>_*` key that holds [`Self::file_path`].
    pub env_key: String,
    /// Name of the Secret.
    pub secret_name: String,
    /// Key inside the Secret, also the file name.
    pub secret_key: String,
    /// Pod-unique volume name.
    pub volume_name: String,
    /// Directory the Secret is mounted at.
    pub mount_path: String,
    /// Full path of the file the pod reads.
    pub file_path: String,
}

/// The flattened config: ConfigMap data and the secret mounts, together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigEnv {
    /// Environment keys and their plain values, or the path of a secret file.
    pub data: BTreeMap<String, String>,
    /// Secret files to mount, in the order of the config.
    pub secret_mounts: Vec<SecretMount>,
}

/// Why a config cannot be flattened.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigEnvError {
    /// The config cannot be serialized to JSON.
    #[error("config cannot be serialized: {message}")]
    Serialize {
        /// The serializer's message.
        message: String,
    },
    /// The config does not serialize to a JSON object.
    #[error("config must serialize to a JSON object")]
    NotAnObject,
    /// A key has a character that cannot be part of an environment variable name.
    #[error(
        "config key `{path}` has a character that cannot be part of an environment variable name"
    )]
    InvalidKey {
        /// The JSON keys of the field, joined with `.`.
        path: String,
    },
    /// Two fields flatten to the same environment key.
    #[error("two config fields flatten to the key `{key}`")]
    DuplicateKey {
        /// The clashing environment key.
        key: String,
    },
    /// A secret reference sits inside a list, which cannot be mounted.
    #[error("secret reference at `{path}` is inside a list; use one field for each secret")]
    SecretRefInList {
        /// The JSON keys of the list, joined with `.`.
        path: String,
    },
}

/// Flattens `config` of profile `P` into `<PREFIX>_*` keys and secret mounts.
pub fn config_env<P: ServiceProfile>(config: &P::Config) -> Result<ConfigEnv, ConfigEnvError>
where
    P::Config: serde::Serialize,
{
    let value = serde_json::to_value(config).map_err(|e| ConfigEnvError::Serialize {
        message: e.to_string(),
    })?;
    let object = match value {
        Value::Object(object) => object,
        _ => return Err(ConfigEnvError::NotAnObject),
    };
    let mut env = ConfigEnv::default();
    let mut keys = Vec::new();
    for (key, value) in &object {
        keys.push(key.clone());
        collect(P::PREFIX, &mut keys, value, &mut env)?;
        keys.pop();
    }
    Ok(env)
}

/// The `data` of a ConfigMap named `name` in `namespace`.
pub fn config_map(name: &str, namespace: &str, env: &ConfigEnv) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {"name": name, "namespace": namespace},
        "data": env.data,
    })
}

/// One `spec.volumes` entry per secret mount.
pub fn secret_volumes(env: &ConfigEnv) -> Vec<Value> {
    env.secret_mounts
        .iter()
        .map(|m| {
            json!({
                "name": m.volume_name,
                "secret": {
                    "secretName": m.secret_name,
                    "defaultMode": READ_ONLY_MODE,
                    "items": [{"key": m.secret_key, "path": m.secret_key}],
                },
            })
        })
        .collect()
}

/// One `volumeMounts` entry per secret mount.
pub fn secret_volume_mounts(env: &ConfigEnv) -> Vec<Value> {
    env.secret_mounts
        .iter()
        .map(|m| {
            json!({
                "name": m.volume_name,
                "mountPath": m.mount_path,
                "readOnly": true,
            })
        })
        .collect()
}

/// The `envFrom` entry that loads the ConfigMap `config_map_name`.
pub fn env_from(config_map_name: &str) -> Value {
    json!({"configMapRef": {"name": config_map_name}})
}

/// Walks one value. `keys` holds the JSON keys that lead to it.
fn collect(
    prefix: &str,
    keys: &mut Vec<String>,
    value: &Value,
    env: &mut ConfigEnv,
) -> Result<(), ConfigEnvError> {
    match value {
        Value::Null => Ok(()),
        Value::Object(object) if is_secret_ref(object) => {
            let key = env_key(prefix, keys)?;
            ensure_new(env, &key)?;
            let secret_name = string_field(object, "secretName");
            let secret_key = string_field(object, "key");
            let mount = secret_mount(&key, &secret_name, &secret_key);
            env.data.insert(key, mount.file_path.clone());
            env.secret_mounts.push(mount);
            Ok(())
        }
        Value::Object(object) => {
            for (key, value) in object {
                keys.push(key.clone());
                collect(prefix, keys, value, env)?;
                keys.pop();
            }
            Ok(())
        }
        Value::Array(_) => {
            if contains_secret_ref(value) {
                return Err(ConfigEnvError::SecretRefInList {
                    path: keys.join("."),
                });
            }
            insert_leaf(prefix, keys, value.to_string(), env)
        }
        Value::String(text) => insert_leaf(prefix, keys, text.clone(), env),
        Value::Bool(_) | Value::Number(_) => insert_leaf(prefix, keys, value.to_string(), env),
    }
}

fn insert_leaf(
    prefix: &str,
    keys: &[String],
    text: String,
    env: &mut ConfigEnv,
) -> Result<(), ConfigEnvError> {
    let key = env_key(prefix, keys)?;
    ensure_new(env, &key)?;
    env.data.insert(key, text);
    Ok(())
}

fn ensure_new(env: &ConfigEnv, key: &str) -> Result<(), ConfigEnvError> {
    if env.data.contains_key(key) || env.secret_mounts.iter().any(|m| m.env_key == key) {
        return Err(ConfigEnvError::DuplicateKey {
            key: key.to_string(),
        });
    }
    Ok(())
}

/// `<PREFIX>_<SEGMENT>_<SEGMENT>...` for the JSON keys `keys`.
fn env_key(prefix: &str, keys: &[String]) -> Result<String, ConfigEnvError> {
    let mut parts = vec![prefix.to_string()];
    for key in keys {
        match env_segment(key) {
            Some(segment) => parts.push(segment),
            None => {
                return Err(ConfigEnvError::InvalidKey {
                    path: keys.join("."),
                })
            }
        }
    }
    Ok(parts.join("_"))
}

/// One JSON key as an upper-case segment: `cacheMb` and `cache_mb` both give `CACHE_MB`.
fn env_segment(key: &str) -> Option<String> {
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in key.chars() {
        let is_word_start = c.is_ascii_uppercase()
            && matches!(previous, Some(p) if p.is_ascii_lowercase() || p.is_ascii_digit());
        if is_word_start {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
        previous = Some(c);
    }
    Some(out)
}

fn secret_mount(env_key: &str, secret_name: &str, secret_key: &str) -> SecretMount {
    let slug = env_key.to_ascii_lowercase().replace('_', "-");
    let mount_path = format!("{SECRET_MOUNT_ROOT}/{slug}");
    SecretMount {
        env_key: env_key.to_string(),
        secret_name: secret_name.to_string(),
        secret_key: secret_key.to_string(),
        volume_name: format!("secret-{slug}"),
        file_path: format!("{mount_path}/{secret_key}"),
        mount_path,
    }
}

/// True for exactly `{"secretName": "...", "key": "..."}`.
fn is_secret_ref(object: &Map<String, Value>) -> bool {
    object.len() == 2
        && matches!(
            (object.get("secretName"), object.get("key")),
            (Some(Value::String(_)), Some(Value::String(_)))
        )
}

fn contains_secret_ref(value: &Value) -> bool {
    match value {
        Value::Object(object) => is_secret_ref(object) || object.values().any(contains_secret_ref),
        Value::Array(items) => items.iter().any(contains_secret_ref),
        _ => false,
    }
}

fn string_field(object: &Map<String, Value>, name: &str) -> String {
    object
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests;
