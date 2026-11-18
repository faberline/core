//! Install manifests of the operator of a profile (ADR 0002 D14): ServiceAccount,
//! ClusterRole, ClusterRoleBinding, CRD and the operator Deployment.
//!
//! Pure: no I/O. The RBAC rules for the rendered children are derived from the kinds
//! of the children that the one renderer produces, so there is no hand-written list.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use super::common::ServicePodTemplate;
use super::deployment::{service_deployment, ServiceDeployment};
use super::derived_rbac::derive_rbac_rules;
use super::instance_render::{render_operator, InstanceRenderError};
use super::rbac::{cluster_role_binding, ClusterRoleBinding, ServiceAccountSubject};
use super::{guaranteed_resources, service_account, RenderCtx};
use crate::domain::profile::{validate_profile, ProfileError, ServiceProfile};
use crate::interfaces::service_spec::{crd_for, ServiceSpec};

/// Where and how the operator is installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallOptions {
    /// Namespace of the operator ServiceAccount and Deployment.
    pub namespace: String,
    /// Container image of the operator.
    pub image: String,
    /// API group of the profile's CRD, for example `lumen.example`.
    pub group: String,
}

/// Why the install manifests could not be rendered.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// The profile is invalid.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// The sample instance could not be rendered.
    #[error(transparent)]
    Render(#[from] InstanceRenderError),
}

/// Verbs on the profile's own CRD kind.
const CRD_VERBS: [&str; 5] = ["get", "list", "watch", "update", "patch"];
/// Verbs on its `/status` subresource.
const STATUS_VERBS: [&str; 3] = ["get", "update", "patch"];
/// Verbs on `leases` (leader election).
const LEASE_VERBS: [&str; 7] = [
    "get", "list", "watch", "create", "update", "patch", "delete",
];
/// Verbs on `events`.
const EVENT_VERBS: [&str; 2] = ["create", "patch"];

fn rule(group: &str, resource: &str, verbs: &[&str]) -> Value {
    json!({ "apiGroups": [group], "resources": [resource], "verbs": verbs })
}

/// The install objects, in this order: `ServiceAccount`, `ClusterRole`, `ClusterRoleBinding`,
/// `CustomResourceDefinition`, `Deployment`.
///
/// `sample` is any valid instance of the profile; only the kinds of the children that
/// `render_operator::<P>` produces for it matter (a monitoring child appears in the rules only
/// when `sample.monitoring.enabled` is true). The ClusterRole holds, first, the derived
/// rules (one per distinct group and resource of the children, from `derive_rbac_rules`), then
/// four fixed rules: the profile's CRD kind, its `/status`, `leases` in `coordination.k8s.io`,
/// and `events`. The ClusterRole and ClusterRoleBinding are cluster-scoped and have no owner
/// reference. The Deployment has one replica and runs as the ServiceAccount.
pub fn operator_install<P>(
    opts: &InstallOptions,
    sample: &ServiceSpec<P::Config>,
) -> Result<Vec<Value>, InstallError>
where
    P: ServiceProfile,
    P::Config: Serialize + JsonSchema + DeserializeOwned,
{
    validate_profile::<P>()?;
    let app = P::KIND.to_ascii_lowercase();
    let name = format!("{app}-operator");
    let api_version = format!("{}/v1", opts.group);
    let operator_cx = RenderCtx::new(&app, &name, &api_version, P::KIND, &name, &opts.namespace);

    // The children of one sample instance; the rules come from their kinds only.
    let sample_cx = RenderCtx::new(
        &app,
        &name,
        &api_version,
        P::KIND,
        "sample",
        &opts.namespace,
    );
    let children = render_operator::<P>(&sample_cx, sample)?;
    let mut rules: Vec<Value> = derive_rbac_rules(&children)
        .iter()
        .map(|derived| rule(&derived.api_group, &derived.resource, &derived.verbs))
        .collect();
    let plural = format!("{app}s");
    rules.push(rule(&opts.group, &plural, &CRD_VERBS));
    rules.push(rule(
        &opts.group,
        &format!("{plural}/status"),
        &STATUS_VERBS,
    ));
    rules.push(rule("coordination.k8s.io", "leases", &LEASE_VERBS));
    rules.push(rule("", "events", &EVENT_VERBS));

    let cluster_role = json!({
        "apiVersion": "rbac.authorization.k8s.io/v1",
        "kind": "ClusterRole",
        "metadata": { "name": name, "labels": operator_cx.labels("operator") },
        "rules": rules,
    });
    let subjects = [ServiceAccountSubject {
        namespace: &opts.namespace,
        name: &name,
    }];
    let binding = cluster_role_binding(ClusterRoleBinding {
        name: &name,
        labels: operator_cx.labels("operator"),
        cluster_role: &name,
        subjects: &subjects,
    });
    let crd = crd_for::<P>(&opts.group)?;
    let deployment = service_deployment(ServiceDeployment {
        name: &name,
        replicas: 1,
        min_ready_seconds: None,
        revision_history_limit: None,
        strategy: None,
        pod: ServicePodTemplate {
            cx: &operator_cx,
            component: "operator",
            image: &opts.image,
            image_pull_policy: "IfNotPresent",
            command: Vec::new(),
            args: Vec::new(),
            ports: Vec::new(),
            env: Vec::new(),
            env_from: Vec::new(),
            resources: guaranteed_resources("100m", "128Mi"),
            readiness_probe: None,
            liveness_probe: None,
            startup_probe: None,
            lifecycle: None,
            container_security_context: None,
            pod_security_context: None,
            service_account_name: Some(&name),
            termination_grace_period_seconds: None,
            volumes: Vec::new(),
            volume_mounts: Vec::new(),
            pod_annotations: None,
            topology_spread_constraints: Vec::new(),
        },
    });
    Ok(vec![
        service_account(&operator_cx, "operator"),
        cluster_role,
        binding,
        crd,
        deployment,
    ])
}

#[cfg(test)]
mod tests;
