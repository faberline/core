//! The typed service spec: the blocks that core owns plus the product's `config` (ADR 0002 D6).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::domain::backup_job::BackupJobPolicy;
use crate::domain::profile::{validate_profile, ProfileError, ServiceProfile};
use crate::infrastructure::crd::normalize_unsigned_integer_formats;
use crate::infrastructure::manifest::placement::Toleration;
use crate::interfaces::cluster_spec::ResourceSpec;

/// The container image of the service.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSpec {
    /// Image reference.
    pub image: String,
    /// Image pull policy, passed through as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<String>,
}

/// The data volume of the service.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageSpec {
    /// Volume size, for example `10Gi`.
    pub size: String,
    /// Storage class name; the cluster default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

/// How many replicas each shard runs and how many of them vote.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplicasSpec {
    /// Replicas per shard.
    #[serde(default = "one")]
    pub replicas_per_shard: u32,
    /// Voting members per shard.
    #[serde(default = "one")]
    pub voters: u32,
}

impl Default for ReplicasSpec {
    fn default() -> Self {
        Self {
            replicas_per_shard: 1,
            voters: 1,
        }
    }
}

/// How clients authenticate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    /// No authentication.
    None,
    /// Bearer tokens from a token registry.
    Token,
}

/// Client authentication.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthSpec {
    /// The authentication mode.
    pub mode: AuthMode,
    /// Name of the Secret that holds the token registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_registry: Option<String>,
}

/// Peer TLS between the replicas.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerTlsSpec {
    /// Whether peers use TLS.
    #[serde(default)]
    pub enabled: bool,
}

/// Scheduled backups.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupSpec {
    /// Cron schedule; no backups when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Retry, deadline and cleanup values for each backup Job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<BackupJobPolicy>,
}

/// Where the pods run.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlacementSpec {
    /// Node labels the pods must match.
    #[serde(default)]
    pub node_selector: BTreeMap<String, String>,
    /// Tolerations for node taints.
    #[serde(default)]
    pub tolerations: Vec<Toleration>,
    /// Kubernetes affinity object, passed through as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "preserve_unknown_object")]
    pub affinity: Option<Value>,
    /// Kubernetes topology spread constraints, passed through as written.
    #[serde(default)]
    #[schemars(schema_with = "preserve_unknown_object_list")]
    pub topology_spread: Vec<Value>,
}

/// Metrics and monitoring objects.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitoringSpec {
    /// Whether the monitoring objects are rendered.
    #[serde(default)]
    pub enabled: bool,
}

/// The spec of a service CRD: the blocks core owns, plus the product's own `config`.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceSpec<C> {
    /// Container image.
    pub image: ImageSpec,
    /// CPU and memory requests.
    #[serde(default)]
    pub resources: ResourceSpec,
    /// Data volume.
    pub storage: StorageSpec,
    /// Replica counts.
    #[serde(default)]
    pub replicas: ReplicasSpec,
    /// Client authentication.
    pub auth: AuthSpec,
    /// Peer TLS.
    #[serde(default)]
    pub peer_tls: PeerTlsSpec,
    /// Scheduled backups.
    #[serde(default)]
    pub backup: BackupSpec,
    /// Scheduling constraints.
    #[serde(default)]
    pub placement: PlacementSpec,
    /// Monitoring objects.
    #[serde(default)]
    pub monitoring: MonitoringSpec,
    /// Names of the image pull Secrets.
    #[serde(default)]
    pub image_pull_secrets: Vec<String>,
    /// The product's own configuration; core does not interpret it.
    pub config: C,
}

fn one() -> u32 {
    1
}

fn preserve_unknown_object(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    serde_json::from_value(json!({
        "type": "object",
        "x-kubernetes-preserve-unknown-fields": true
    }))
    .expect("static schema is valid")
}

fn preserve_unknown_object_list(
    _: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    serde_json::from_value(json!({
        "type": "array",
        "items": {"type": "object", "x-kubernetes-preserve-unknown-fields": true}
    }))
    .expect("static schema is valid")
}

/// Build the CustomResourceDefinition of profile `P` in `group`, as JSON.
///
/// The kind comes from `P::KIND`. The schema holds the core spec blocks and
/// `spec.config` typed as `P::Config`. Unsigned integers are normalized for
/// Kubernetes structural schemas.
pub fn crd_for<P>(group: &str) -> Result<Value, ProfileError>
where
    P: ServiceProfile,
    P::Config: JsonSchema + Serialize + serde::de::DeserializeOwned,
{
    validate_profile::<P>()?;

    let settings = schemars::gen::SchemaSettings::openapi3().with(|s| {
        s.inline_subschemas = true;
    });
    let generator = settings.into_generator();
    let root = generator.into_root_schema_for::<ServiceSpec<P::Config>>();
    let mut spec = serde_json::to_value(root.schema).expect("schema serializes");
    normalize_unsigned_integer_formats(&mut spec);
    if let Value::Object(map) = &mut spec {
        map.remove("$schema");
        map.remove("title");
        map.remove("definitions");
    }

    let singular = P::KIND.to_ascii_lowercase();
    let plural = format!("{singular}s");
    let metadata_name = format!("{plural}.{group}");
    let list_kind = format!("{}List", P::KIND);
    Ok(json!({
        "apiVersion": "apiextensions.k8s.io/v1",
        "kind": "CustomResourceDefinition",
        "metadata": {"name": metadata_name},
        "spec": {
            "group": group,
            "names": {
                "kind": P::KIND,
                "plural": plural,
                "singular": singular,
                "listKind": list_kind
            },
            "scope": "Namespaced",
            "versions": [{
                "name": "v1",
                "served": true,
                "storage": true,
                "subresources": {"status": {}},
                "schema": {"openAPIV3Schema": {
                    "type": "object",
                    "properties": {
                        "spec": spec,
                        "status": {"type": "object", "x-kubernetes-preserve-unknown-fields": true}
                    }
                }}
            }]
        }
    }))
}

#[cfg(test)]
mod tests;
