//! The shared CRD spec fragments a sharded-HA service embeds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The generic cluster shape every sharded-HA service embeds in its CRD spec via
/// `#[serde(flatten)] pub cluster: service_k8s::ClusterSpec`.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSpec {
    pub image: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<String>,
    #[serde(default = "one")]
    pub shard_count: u32,
    /// Desired members per shard. Changing it is a Raft membership transition
    /// (add learner, promote, demote, remove) that the controller completes one
    /// step at a time before it changes the pod count.
    #[serde(default = "one")]
    pub replicas_per_shard: u32,
    #[serde(default = "one")]
    pub voter_count: u32,
    #[serde(default)]
    pub resources: ResourceSpec,
}

/// Per-pod CPU/memory requests. Empty values resolve to the shared data-plane
/// defaults (`1` CPU / `4Gi`) at render time. Limits are intentionally omitted
/// so a dedicated-node pod can use otherwise-idle node capacity.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpec {
    #[serde(default)]
    pub cpu: String,
    #[serde(default)]
    pub memory: String,
}

fn one() -> u32 {
    1
}
