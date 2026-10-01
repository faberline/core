//! Service-neutral rendering for one durable StatefulSet instance.
//!
//! This module contains no service policy.  Callers provide the identity and
//! an already composed pod template; the renderer only attaches storage and
//! emits the Kubernetes StatefulSet shape.  Keeping this seam in the shared
//! kit lets Standalone and Managed/Fleet use the same stateful contract.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use thiserror::Error;

use super::{
    common::ServicePodTemplate, RenderCtx, ServiceStatefulSet, ENV_POD_NAME, ENV_POD_NAMESPACE,
    ENV_REPLICAS_PER_SHARD, ENV_SHARD_COUNT, ENV_VOTER_COUNT,
};

mod renderer;

pub use renderer::stateful_instance;

/// Compatibility adapter for the historical render root.  The root API stays
/// source-compatible while the stateful shape is assembled by this module.
pub fn render_compat_service_statefulset(p: ServiceStatefulSet) -> Value {
    render_compat_service_statefulset_impl(p, None)
}

pub(super) fn render_compat_service_statefulset_with_links(
    p: ServiceStatefulSet,
    enable_service_links: bool,
) -> Value {
    render_compat_service_statefulset_impl(p, Some(enable_service_links))
}

fn render_compat_service_statefulset_impl(
    p: ServiceStatefulSet,
    enable_service_links: Option<bool>,
) -> Value {
    let ServiceStatefulSet {
        cx,
        name,
        component,
        image,
        image_pull_policy,
        command,
        args,
        ports,
        headless_service,
        shard_count,
        replicas_per_shard,
        voter_count,
        headless_env_key,
        service_account_name,
        env: extra_env,
        env_from,
        resources,
        pod_annotations,
        pod_security_context,
        container_security_context,
        termination_grace_period_seconds,
        readiness_probe,
        liveness_probe,
        startup_probe,
        lifecycle,
        volumes,
        volume_mounts,
        affinity,
        node_selector,
        tolerations,
        topology_spread_constraints,
        revision_history_limit,
        update_strategy,
        volume_claim,
    } = p;

    let mut env = vec![
        json!({ "name": ENV_POD_NAME, "valueFrom": { "fieldRef": { "fieldPath": "metadata.name" } } }),
        json!({ "name": ENV_POD_NAMESPACE, "valueFrom": { "fieldRef": { "fieldPath": "metadata.namespace" } } }),
        json!({ "name": ENV_SHARD_COUNT, "value": shard_count.to_string() }),
        json!({ "name": ENV_REPLICAS_PER_SHARD, "value": replicas_per_shard.to_string() }),
        json!({ "name": ENV_VOTER_COUNT, "value": voter_count.to_string() }),
        json!({ "name": headless_env_key, "value": headless_service }),
    ];
    env.extend(extra_env);

    let pod = ServicePodTemplate {
        cx,
        component,
        image,
        image_pull_policy,
        command,
        args,
        ports,
        env,
        env_from,
        resources,
        readiness_probe,
        liveness_probe,
        startup_probe,
        lifecycle,
        container_security_context,
        pod_security_context,
        service_account_name,
        termination_grace_period_seconds,
        volumes,
        volume_mounts,
        pod_annotations,
        topology_spread_constraints: vec![],
    };
    let storage = volume_claim.map(|claim| {
        StatefulStorageAttachment::VolumeClaimTemplate(VolumeClaimTemplate {
            name: claim.name,
            template: claim.template,
            mount_path: claim.mount_path.to_owned(),
            read_only: claim.read_only,
        })
    });
    let mut plan = StatefulInstancePlan::without_storage(
        cx,
        headless_service,
        shard_count * replicas_per_shard,
        pod,
    );
    plan.name = name.into();
    plan.storage = storage;
    plan.affinity = affinity;
    plan.node_selector = node_selector;
    plan.tolerations = tolerations;
    plan.topology_spread_constraints = topology_spread_constraints;
    plan.revision_history_limit = revision_history_limit;
    plan.update_strategy = update_strategy;
    plan.enable_service_links = enable_service_links;
    let rendered = stateful_instance(plan).expect("validated compatibility stateful plan");
    rendered.workload
}

/// A StatefulSet-managed PVC template and the mount that consumes it.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeClaimTemplate {
    pub name: String,
    pub template: Value,
    pub mount_path: String,
    pub read_only: bool,
}

impl VolumeClaimTemplate {
    pub fn new(name: impl Into<String>, template: Value, mount_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template,
            mount_path: mount_path.into(),
            read_only: false,
        }
    }
}

/// An independently rendered PVC and the pod volume that consumes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExistingClaim {
    pub volume_name: String,
    pub claim_name: String,
    pub template: Value,
    pub mount_path: String,
    pub read_only: bool,
}

impl ExistingClaim {
    pub fn new(
        volume_name: impl Into<String>,
        claim_name: impl Into<String>,
        template: Value,
        mount_path: impl Into<String>,
    ) -> Self {
        Self {
            volume_name: volume_name.into(),
            claim_name: claim_name.into(),
            template,
            mount_path: mount_path.into(),
            read_only: false,
        }
    }
}

/// How the instance obtains its durable volume.
#[derive(Clone, Debug, PartialEq)]
pub enum StatefulStorageAttachment {
    /// Kubernetes creates one claim per StatefulSet member.
    VolumeClaimTemplate(VolumeClaimTemplate),
    /// Kubernetes binds the pod to an independently managed claim.
    ExistingClaim(ExistingClaim),
}

/// Input to [`stateful_instance`].  `pod_template` is the complete
/// `spec.template` object, so service-specific image, ports, probes, security
/// and environment remain owned by the service.
pub struct StatefulInstancePlan<'a> {
    pub cx: &'a RenderCtx<'a>,
    /// The StatefulSet metadata name. The context name remains the instance
    /// identity used in labels and owner metadata.
    pub name: String,
    pub service_name: String,
    pub replicas: u32,
    pub selector: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub pod: ServicePodTemplate<'a>,
    /// No attachment leaves the workload without a PVC, mount, or volume.
    pub storage: Option<StatefulStorageAttachment>,
    pub pod_management_policy: Option<String>,
    pub affinity: Option<Value>,
    pub node_selector: Option<Value>,
    pub tolerations: Vec<Value>,
    pub topology_spread_constraints: Vec<Value>,
    /// Explicitly control Kubernetes service-link environment injection.
    /// `None` preserves the historical default; service profiles may set it.
    pub enable_service_links: Option<bool>,
    pub revision_history_limit: Option<i32>,
    pub update_strategy: Option<Value>,
}

impl<'a> StatefulInstancePlan<'a> {
    pub fn new(
        cx: &'a RenderCtx<'a>,
        service_name: impl Into<String>,
        replicas: u32,
        pod: ServicePodTemplate<'a>,
        storage: StatefulStorageAttachment,
    ) -> Self {
        Self::with_optional_storage(cx, service_name, replicas, pod, Some(storage))
    }

    /// Build an instance with no durable attachment.
    pub fn without_storage(
        cx: &'a RenderCtx<'a>,
        service_name: impl Into<String>,
        replicas: u32,
        pod: ServicePodTemplate<'a>,
    ) -> Self {
        Self::with_optional_storage(cx, service_name, replicas, pod, None)
    }

    fn with_optional_storage(
        cx: &'a RenderCtx<'a>,
        service_name: impl Into<String>,
        replicas: u32,
        pod: ServicePodTemplate<'a>,
        storage: Option<StatefulStorageAttachment>,
    ) -> Self {
        Self {
            cx,
            name: cx.name().into(),
            service_name: service_name.into(),
            replicas,
            selector: BTreeMap::new(),
            labels: BTreeMap::new(),
            pod,
            storage,
            pod_management_policy: Some("Parallel".into()),
            affinity: None,
            node_selector: None,
            tolerations: Vec::new(),
            topology_spread_constraints: Vec::new(),
            enable_service_links: None,
            revision_history_limit: None,
            update_strategy: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatefulInstanceRender {
    /// Present only for [`StatefulStorageAttachment::ExistingClaim`]. Apply it
    /// before [`Self::workload`] so the pod volume has a claim to bind.
    pub storage: Option<Value>,
    /// The StatefulSet workload, rendered after optional independent storage.
    pub workload: Value,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatefulInstanceError {
    #[error("replicas must be greater than zero")]
    ZeroReplicas,
    #[error("identity must not be empty")]
    EmptyIdentity,
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    #[error("volume and mount names must be unique")]
    VolumeMountCollision,
    #[error("selector and core identity labels are immutable")]
    SelectorCoreIdentityOverride,
    #[error("pod template must be an object")]
    InvalidPodTemplate,
}

#[cfg(test)]
mod tests;
