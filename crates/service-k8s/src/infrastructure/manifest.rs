//! The sharded-HA render toolkit: a [`RenderCtx`] carrying the per-service
//! identity (app/manager/GVK/name/ns/owner) plus helpers that emit the common
//! k8s objects — labels/selector/meta, ServiceAccount, headless + client
//! Services, PodDisruptionBudget, CronJobs, and [`sharded_statefulset`]: the
//! downward-API StatefulSet whose env feeds
//! `raft_runtime::cluster::ClusterTopology::from_env`.
//!
//! Lifted + parameterized from lumen's `service_k8s::render` helpers. A service
//! keeps its own service-specific rendering and calls these for the shared
//! shapes.
//!
//! The layout is lopsided on purpose (#1849). Pod composition and the stateless
//! Deployment shape live in [`common`] and [`deployment`], but the StatefulSet
//! helpers and the ordinary children stayed at the root: their callers were
//! already deployed against these paths, and moving them would have made a
//! rendering refactor into a breaking change for every adopter at once. So a
//! helper's depth here records when it arrived, not how shared it is — do not
//! read the root as legacy, and do not "finish" the split without moving the
//! callers in the same change.

use serde_json::{json, Value};

use crate::domain::capacity::{
    resource_request_or_default, DEFAULT_CPU_REQUEST, DEFAULT_MEMORY_REQUEST,
};

// The downward-API env keys a sharded-HA StatefulSet injects. These MUST match
// `raft_runtime::cluster::ClusterTopology::from_env` (the consumer) — duplicated
// here (rather than depending on raft-runtime) to keep this kube-only lib free of
// the raftcore/h2c/reqwest dep tree; the `downward_api_env_keys` test asserts
// `sharded_statefulset` emits exactly these.
pub const ENV_POD_NAME: &str = "POD_NAME";
pub const ENV_POD_NAMESPACE: &str = "POD_NAMESPACE";
pub const ENV_SHARD_COUNT: &str = "SHARD_COUNT";
pub const ENV_REPLICAS_PER_SHARD: &str = "REPLICAS_PER_SHARD";
pub const ENV_VOTER_COUNT: &str = "VOTER_COUNT";

pub mod common;
pub mod deployment;
pub mod projected_token;
pub mod rbac;
pub mod stateful_instance;
pub mod workload_plan;
mod workloads;

pub use workload_plan::*;
pub use workloads::{
    cron_job, horizontal_pod_autoscaler, service_statefulset,
    service_statefulset_with_service_links, sharded_statefulset, CronJob, HorizontalPodAutoscaler,
    ServiceStatefulSet, ShardedStatefulSet,
};

/// Per-service render identity, threaded through the helpers.
pub struct RenderCtx<'a> {
    pub app: &'a str,
    pub manager: &'a str,
    pub api_version: &'a str,
    pub kind: &'a str,
    pub name: &'a str,
    pub ns: &'a str,
    pub owner: Option<Value>,
}

impl RenderCtx<'_> {
    /// Recommended labels common to every child object.
    pub fn labels(&self, component: &str) -> Value {
        json!({
            "app.kubernetes.io/name": self.app,
            "app.kubernetes.io/instance": self.name,
            "app.kubernetes.io/component": component,
            "app.kubernetes.io/managed-by": self.manager,
            "app.kubernetes.io/part-of": self.app,
        })
    }

    /// Immutable selector labels (a subset of [`Self::labels`]) — workload and
    /// Service selectors pin to these so re-applies never hit selector-immutability.
    pub fn selector(&self, component: &str) -> Value {
        json!({
            "app.kubernetes.io/name": self.app,
            "app.kubernetes.io/instance": self.name,
            "app.kubernetes.io/component": component,
        })
    }

    /// Assemble an object's `metadata` block (name/ns/labels + owner ref).
    pub fn meta(&self, name: &str, component: &str) -> Value {
        let mut m = json!({ "name": name, "namespace": self.ns, "labels": self.labels(component) });
        if let Some(o) = &self.owner {
            m["ownerReferences"] = json!([o]);
        }
        m
    }
}

/// The owner reference that ties a child to its CR (cascading GC). `uid` comes
/// from the live CR's metadata.
pub fn owner_ref(api_version: &str, kind: &str, name: &str, uid: &str) -> Value {
    json!({
        "apiVersion": api_version,
        "kind": kind,
        "name": name,
        "uid": uid,
        "controller": true,
        "blockOwnerDeletion": true,
    })
}

/// Guaranteed-QoS CPU/memory resources (`requests == limits`).
pub fn guaranteed_resources(cpu: &str, memory: &str) -> Value {
    json!({
        "requests": { "cpu": cpu, "memory": memory },
        "limits": { "cpu": cpu, "memory": memory },
    })
}

/// Request-only resources for a long-running data pod. Empty inputs use the
/// shared `1` CPU / `4Gi` baseline; no limit is emitted, allowing the one-pod-
/// per-node workload to consume otherwise-idle node capacity.
pub fn requested_resources(cpu: &str, memory: &str) -> Value {
    let cpu = resource_request_or_default(cpu, DEFAULT_CPU_REQUEST);
    let memory = resource_request_or_default(memory, DEFAULT_MEMORY_REQUEST);
    json!({
        "requests": { "cpu": cpu, "memory": memory },
    })
}

/// Required hostname anti-affinity for a dedicated stateful data plane. This
/// is the scheduler-level one-pod-per-node contract; Cluster Autoscaler can
/// react to the Pending pods without the operator inspecting provider-specific
/// node-pool APIs.
pub fn dedicated_node_affinity(selector: Value) -> Value {
    json!({
        "podAntiAffinity": {
            "requiredDuringSchedulingIgnoredDuringExecution": [{
                "labelSelector": { "matchLabels": selector },
                "topologyKey": "kubernetes.io/hostname",
            }],
        },
    })
}

/// Pod-level Kubernetes restricted-profile baseline for long-running service
/// workloads. The matching container-level restrictions come from
/// [`restricted_container_security_context`].
pub fn restricted_pod_security_context() -> Value {
    json!({
        "runAsNonRoot": true,
        "runAsUser": 65532,
        "runAsGroup": 65532,
        "fsGroup": 65532,
        "seccompProfile": { "type": "RuntimeDefault" },
    })
}

/// Container-level Kubernetes restricted-profile baseline for a service
/// workload. Services still add explicit writable `emptyDir` mounts for paths
/// such as `/tmp` when their process needs them.
pub fn restricted_container_security_context() -> Value {
    json!({
        "runAsNonRoot": true,
        "runAsUser": 65532,
        "runAsGroup": 65532,
        "allowPrivilegeEscalation": false,
        "readOnlyRootFilesystem": true,
        "capabilities": { "drop": ["ALL"] },
    })
}

/// A rendered PVC template plus the container mount path it should back.
pub struct WorkloadVolumeClaim<'a> {
    pub name: String,
    pub template: Value,
    pub mount_path: &'a str,
    pub read_only: bool,
}

/// The Secrets Store CSI driver name registered by the vanilla/community
/// installation (the `secrets-store-csi-driver` upstream project). Cloud-managed
/// add-ons can register a different driver name — see
/// [`TokenRegistrySource::Csi`].
pub const DEFAULT_TOKEN_REGISTRY_CSI_DRIVER: &str = "secrets-store.csi.k8s.io";

/// Source for a bearer-token registry projection. A service can use a normal
/// Kubernetes Secret or delegate material delivery to the Secrets Store CSI
/// driver without re-implementing the volume shape in every operator.
pub enum TokenRegistrySource<'a> {
    Secret {
        name: &'a str,
        key: &'a str,
    },
    Csi {
        provider_class: &'a str,
        /// CSI driver name to register on the volume. `None` uses
        /// [`DEFAULT_TOKEN_REGISTRY_CSI_DRIVER`], the vanilla community
        /// driver name. GKE's managed Secrets Store add-on registers
        /// `secrets-store-gke.csi.k8s.io` instead, so GKE instances must
        /// override this (refs #2456/#2457).
        driver: Option<&'a str>,
    },
}

/// One read-only token-registry volume and its corresponding container mount.
/// Services retain their external env names and mount paths; this helper owns
/// the common Kubernetes projection contract.
pub struct TokenRegistryProjection<'a> {
    pub volume_name: &'a str,
    pub mount_path: &'a str,
    pub source: TokenRegistrySource<'a>,
}

/// Render the read-only container mount for a token registry projection.
pub fn token_registry_mount(projection: &TokenRegistryProjection<'_>) -> Value {
    json!({
        "name": projection.volume_name,
        "mountPath": projection.mount_path,
        "readOnly": true,
    })
}

/// Render the Kubernetes volume backing a token registry projection.
pub fn token_registry_volume(projection: &TokenRegistryProjection<'_>) -> Value {
    let mut volume = json!({ "name": projection.volume_name });
    match &projection.source {
        TokenRegistrySource::Secret { name, key } => {
            volume["secret"] = json!({
                "secretName": name,
                "items": [{ "key": key, "path": key }],
            });
        }
        TokenRegistrySource::Csi {
            provider_class,
            driver,
        } => {
            volume["csi"] = json!({
                "driver": driver.unwrap_or(DEFAULT_TOKEN_REGISTRY_CSI_DRIVER),
                "readOnly": true,
                "volumeAttributes": { "secretProviderClass": provider_class },
            });
        }
    }
    volume
}

/// A ServiceAccount for the workload pods.
pub fn service_account(cx: &RenderCtx, component: &str) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "ServiceAccount",
        "metadata": cx.meta(cx.name, component),
    })
}

fn service(
    cx: &RenderCtx,
    name: &str,
    component: &str,
    ports: Vec<Value>,
    cluster_ip: Option<&str>,
    publish_not_ready_addresses: bool,
    service_type: Option<&str>,
) -> Value {
    let mut spec = json!({
        "selector": cx.selector(component),
        "ports": ports,
    });
    if let Some(cluster_ip) = cluster_ip {
        spec["clusterIP"] = json!(cluster_ip);
    }
    if publish_not_ready_addresses {
        spec["publishNotReadyAddresses"] = json!(true);
    }
    if let Some(service_type) = service_type {
        spec["type"] = json!(service_type);
    }
    json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": cx.meta(name, component),
        "spec": spec,
    })
}

/// A headless Service with caller-supplied ports.
pub fn headless_service_with_ports(
    cx: &RenderCtx,
    name: &str,
    component: &str,
    ports: Vec<Value>,
) -> Value {
    service(cx, name, component, ports, Some("None"), true, None)
}

/// A headless Service (stable per-pod DNS for a StatefulSet's peers).
pub fn headless_service(cx: &RenderCtx, name: &str, component: &str, port: i32) -> Value {
    headless_service_with_ports(
        cx,
        name,
        component,
        vec![json!({ "name": "http", "port": port, "targetPort": "http", "protocol": "TCP" })],
    )
}

/// A ClusterIP Service with caller-supplied ports.
pub fn client_service_with_ports(
    cx: &RenderCtx,
    name: &str,
    component: &str,
    ports: Vec<Value>,
) -> Value {
    service(cx, name, component, ports, None, false, Some("ClusterIP"))
}

/// A ClusterIP client Service.
pub fn client_service(cx: &RenderCtx, name: &str, component: &str, port: i32) -> Value {
    client_service_with_ports(
        cx,
        name,
        component,
        vec![json!({ "name": "http", "port": port, "targetPort": "http", "protocol": "TCP" })],
    )
}

/// A PodDisruptionBudget.
pub fn pdb(cx: &RenderCtx, name: &str, component: &str, max_unavailable: i32) -> Value {
    json!({
        "apiVersion": "policy/v1",
        "kind": "PodDisruptionBudget",
        "metadata": cx.meta(name, component),
        "spec": { "maxUnavailable": max_unavailable, "selector": { "matchLabels": cx.selector(component) } },
    })
}

#[cfg(test)]
mod tests;
