//! Render the children of one profile instance for one shard (ADR 0002 D5, D6, D14).
//!
//! Pure: no I/O, no clock. The children come out as plain JSON values, in the order
//! in which they must be applied. The first release supports one shard.

use serde::Serialize;
use serde_json::{json, Value};

use super::common::{network_policy, NetworkPolicy};
use super::stateful_instance::StatefulInstanceError;
use super::AlertRule as BorrowedAlert;
use super::{
    client_service, headless_service, pdb, prometheus_rule, requested_resources, service_monitor,
    service_statefulset, PrometheusRule, RenderCtx, ServiceMonitor, ServiceStatefulSet,
    WorkloadVolumeClaim,
};
use super::{
    ContainerPlan, CronJobPlan, DaemonSetPlan, DeploymentPlan, PersistentVolumeClaimPlan, PodPlan,
    PodRuntimePolicy, StatefulSetPlan, WorkloadPlan, WorkloadPlanError,
};
use crate::domain::config_env::{
    config_env, config_map, env_from, secret_volume_mounts, secret_volumes, ConfigEnvError,
};
use crate::domain::profile::{validate_profile, ProfileError, ServiceProfile, WorkloadKind};
use crate::interfaces::service_spec::ServiceSpec;

/// The `app.kubernetes.io/component` label of every child of an instance.
pub const INSTANCE_COMPONENT: &str = "server";
/// The one port of the instance: the container port named `http` and the port of both Services.
pub const INSTANCE_HTTP_PORT: i32 = 8080;
/// The name of the data volume claim of the StatefulSet.
pub const INSTANCE_DATA_VOLUME: &str = "data";
/// Where the data volume is mounted in the container.
pub const INSTANCE_DATA_MOUNT_PATH: &str = "/data";
/// The fixed schedule of an extra CronJob. Part 1 declares no schedule.
pub const EXTRA_CRONJOB_SCHEDULE: &str = "0 3 * * *";

/// Why an instance is not rendered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum InstanceRenderError {
    /// The profile is refused by `validate_profile`.
    #[error("{0}")]
    Profile(ProfileError),
    /// The config cannot be flattened to `<PREFIX>_*` keys.
    #[error("{0}")]
    Config(ConfigEnvError),
    /// Only one shard is supported in the first release. `shards == 0` is refused too.
    #[error("the first release supports one shard, got {shards}")]
    MultiShardUnsupported {
        /// The refused shard count.
        shards: u32,
    },
    /// The StatefulSet renderer refused the plan, for example `replicas_per_shard == 0`.
    #[error("{0}")]
    Workload(StatefulInstanceError),
    /// `render_static` was called with `replicas == 0`.
    #[error("replicas must be at least 1")]
    NoReplicas,
    /// An extra workload of the profile cannot be planned.
    #[error("{0}")]
    ExtraWorkload(WorkloadPlanError),
}

impl From<ProfileError> for InstanceRenderError {
    fn from(error: ProfileError) -> Self {
        Self::Profile(error)
    }
}

impl From<ConfigEnvError> for InstanceRenderError {
    fn from(error: ConfigEnvError) -> Self {
        Self::Config(error)
    }
}

impl From<StatefulInstanceError> for InstanceRenderError {
    fn from(error: StatefulInstanceError) -> Self {
        Self::Workload(error)
    }
}

/// Severity of the standard alerts.
const STANDARD_ALERT_SEVERITY: &str = "warning";

/// A standard alert of core. `{kind}`, `{ns}` and `{name}` in `expr` and `summary` are replaced
/// with the profile kind, the instance namespace and the instance name.
struct StandardAlert {
    suffix: &'static str,
    expr: &'static str,
    for_duration: &'static str,
    summary: &'static str,
}

/// The standard alerts of core, in rendering order. The alert name is `<Kind><suffix>`.
const STANDARD_ALERTS: [StandardAlert; 2] = [
    StandardAlert {
        suffix: "NotReady",
        expr: "kube_statefulset_status_replicas_ready{namespace=\"{ns}\",statefulset=\"{name}\"} < kube_statefulset_replicas{namespace=\"{ns}\",statefulset=\"{name}\"}",
        for_duration: "10m",
        summary: "{kind} {name} has pods that are not ready",
    },
    StandardAlert {
        suffix: "StorageDegraded",
        expr: "kubelet_volume_stats_available_bytes{namespace=\"{ns}\",persistentvolumeclaim=~\"data-{name}-.*\"} / kubelet_volume_stats_capacity_bytes{namespace=\"{ns}\",persistentvolumeclaim=~\"data-{name}-.*\"} < 0.1",
        for_duration: "10m",
        summary: "{kind} {name} has less than 10% free storage",
    },
];

/// One alert with owned text, so that `BorrowedAlert` can borrow it.
struct OwnedAlert {
    alert: String,
    expr: String,
    for_duration: String,
    severity: String,
    summary: String,
}

/// The standard alerts of core for this instance, then the alerts of the profile, in order.
fn alert_rules<P: ServiceProfile>(ctx: &RenderCtx<'_>) -> Vec<OwnedAlert> {
    let fill = |text: &str| {
        text.replace("{kind}", P::KIND)
            .replace("{ns}", ctx.ns())
            .replace("{name}", ctx.name())
    };
    let mut rules: Vec<OwnedAlert> = STANDARD_ALERTS
        .iter()
        .map(|standard| OwnedAlert {
            alert: format!("{}{}", P::KIND, standard.suffix),
            expr: fill(standard.expr),
            for_duration: standard.for_duration.to_string(),
            severity: STANDARD_ALERT_SEVERITY.to_string(),
            summary: fill(standard.summary),
        })
        .collect();
    rules.extend(P::alerts().into_iter().map(|rule| OwnedAlert {
        alert: rule.name,
        expr: rule.expression,
        for_duration: rule.for_duration,
        severity: rule.severity,
        summary: rule.summary,
    }));
    rules
}

/// Render the children of one profile instance for one shard, in this order:
/// ConfigMap `<name>-config`, headless Service `<name>-headless`, client Service
/// `<name>`, StatefulSet `<name>`, PodDisruptionBudget `<name>`, NetworkPolicy `<name>`
/// (`<name>` is `ctx.name()`). It does not render a ServiceAccount, so the pods use the
/// namespace's default one. With `spec.monitoring.enabled` it appends a ServiceMonitor
/// `<name>` and a PrometheusRule `<name>-alerts`. Then, for each `P::extra_workloads()` in
/// declaration order, the objects of that workload (see `extra_workload_children`).
///
/// `validate_profile::<P>()` runs first, then the shard check (`shards != 1` is
/// `MultiShardUnsupported`), then `config_env::<P>`.
pub fn render_instance<P>(
    ctx: &RenderCtx<'_>,
    spec: &ServiceSpec<P::Config>,
    shards: u32,
) -> Result<Vec<Value>, InstanceRenderError>
where
    P: ServiceProfile,
    P::Config: Serialize,
{
    validate_profile::<P>()?;
    if shards != 1 {
        return Err(InstanceRenderError::MultiShardUnsupported { shards });
    }
    let env = config_env::<P>(&spec.config)?;

    let name = ctx.name();
    let component = INSTANCE_COMPONENT;
    let config_map_name = format!("{name}-config");
    let headless_name = format!("{name}-headless");
    let headless_env_key = format!("{}_HEADLESS_SERVICE", P::PREFIX);

    // The ConfigMap helper writes a bare `metadata`; use the context's so the labels and
    // the owner reference match the other children.
    let mut config = config_map(&config_map_name, ctx.ns(), &env);
    config["metadata"] = ctx.meta(&config_map_name, component);

    let mut claim = json!({
        "accessModes": ["ReadWriteOnce"],
        "resources": { "requests": { "storage": spec.storage.size } },
    });
    if let Some(class) = &spec.storage.class {
        claim["storageClassName"] = json!(class);
    }
    let volume_claim = WorkloadVolumeClaim {
        name: INSTANCE_DATA_VOLUME.to_string(),
        template: json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": { "name": INSTANCE_DATA_VOLUME },
            "spec": claim,
        }),
        mount_path: INSTANCE_DATA_MOUNT_PATH,
        read_only: false,
    };

    let statefulset = service_statefulset(ServiceStatefulSet {
        cx: ctx,
        name,
        component,
        image: &spec.image.image,
        image_pull_policy: spec
            .image
            .image_pull_policy
            .as_deref()
            .unwrap_or("IfNotPresent"),
        command: Vec::new(),
        args: Vec::new(),
        ports: Vec::from([
            json!({ "name": "http", "containerPort": INSTANCE_HTTP_PORT, "protocol": "TCP" }),
        ]),
        headless_service: &headless_name,
        shard_count: shards,
        replicas_per_shard: spec.replicas.replicas_per_shard,
        voter_count: spec.replicas.voters,
        headless_env_key: &headless_env_key,
        service_account_name: None,
        env: Vec::new(),
        env_from: Vec::from([env_from(&config_map_name)]),
        resources: requested_resources(&spec.resources.cpu, &spec.resources.memory),
        pod_annotations: None,
        pod_security_context: None,
        container_security_context: None,
        termination_grace_period_seconds: None,
        readiness_probe: None,
        liveness_probe: None,
        startup_probe: None,
        lifecycle: None,
        volumes: secret_volumes(&env),
        volume_mounts: secret_volume_mounts(&env),
        affinity: None,
        node_selector: None,
        tolerations: Vec::new(),
        topology_spread_constraints: Vec::new(),
        revision_history_limit: None,
        update_strategy: None,
        volume_claim: Some(volume_claim),
    })?;

    let mut children = Vec::from([
        config,
        headless_service(ctx, &headless_name, component, INSTANCE_HTTP_PORT),
        client_service(ctx, name, component, INSTANCE_HTTP_PORT),
        statefulset,
        pdb(ctx, name, component, 1),
        network_policy(NetworkPolicy {
            cx: ctx,
            name,
            component,
            client_ports: Vec::from([INSTANCE_HTTP_PORT]),
            peer_ports: Vec::from([INSTANCE_HTTP_PORT]),
            extra_egress: Vec::new(),
        }),
    ]);

    if spec.monitoring.enabled {
        children.push(service_monitor(ServiceMonitor {
            cx: ctx,
            name,
            component,
            port: "http",
            path: "/metrics",
            interval: "30s",
        }));
        let owned = alert_rules::<P>(ctx);
        let rules = owned
            .iter()
            .map(|rule| BorrowedAlert {
                alert: &rule.alert,
                expr: &rule.expr,
                for_duration: &rule.for_duration,
                severity: &rule.severity,
                summary: &rule.summary,
            })
            .collect();
        children.push(prometheus_rule(PrometheusRule {
            cx: ctx,
            name: &format!("{name}-alerts"),
            component,
            group: name,
            rules,
        }));
    }
    children.extend(
        extra_workload_children::<P>(ctx, spec, &config_map_name, &headless_name)
            .map_err(InstanceRenderError::ExtraWorkload)?,
    );
    Ok(children)
}

/// The objects of the profile's extra workloads, in declaration order: one object per
/// workload, named `<name>-<workload>`, with the instance's image, resources and config.
fn extra_workload_children<P: ServiceProfile>(
    ctx: &RenderCtx<'_>,
    spec: &ServiceSpec<P::Config>,
    config_map_name: &str,
    headless_name: &str,
) -> Result<Vec<Value>, WorkloadPlanError> {
    let mut plan = WorkloadPlan::new(ctx);
    for workload in P::extra_workloads() {
        let name = format!("{}-{}", ctx.name(), workload.name);
        let component = workload.name.as_str();
        let mut container =
            ContainerPlan::new(workload.name.clone(), spec.image.image.clone(), Vec::new());
        container.image_pull_policy = Some(
            spec.image
                .image_pull_policy
                .clone()
                .unwrap_or_else(|| "IfNotPresent".to_string()),
        );
        container.env_from = Vec::from([env_from(config_map_name)]);
        container.resources = Some(requested_resources(
            &spec.resources.cpu,
            &spec.resources.memory,
        ));
        let pod = PodPlan::new(
            component,
            container,
            PodRuntimePolicy::restricted("default", json!({})),
        );
        match workload.kind {
            WorkloadKind::StatefulSet => plan.add_stateful_set(StatefulSetPlan::new(
                name,
                headless_name,
                1,
                pod,
                PersistentVolumeClaimPlan::new(
                    INSTANCE_DATA_VOLUME,
                    component,
                    spec.storage.size.clone(),
                    INSTANCE_DATA_MOUNT_PATH,
                ),
            )),
            WorkloadKind::Deployment => plan.add_deployment(DeploymentPlan::new(name, 1, pod)),
            WorkloadKind::DaemonSet => plan.add_daemon_set(DaemonSetPlan::new(name, pod)),
            WorkloadKind::CronJob => {
                plan.add_cron_job(CronJobPlan::new(name, EXTRA_CRONJOB_SCHEDULE, pod))
            }
        }
    }
    plan.render()
}

/// Render one profile instance for the operator: the one renderer with one shard.
pub fn render_operator<P>(
    ctx: &RenderCtx<'_>,
    spec: &ServiceSpec<P::Config>,
) -> Result<Vec<Value>, InstanceRenderError>
where
    P: ServiceProfile,
    P::Config: Serialize,
{
    render_instance::<P>(ctx, spec, 1)
}

/// Render one profile instance for the static path with `replicas` pods.
///
/// It copies the spec, sets `replicas_per_shard = replicas` and
/// `voters = replicas.min(spec.replicas.voters)`, and calls `render_instance` with one shard.
/// The input spec is not changed. `replicas == 0` is `NoReplicas`.
pub fn render_static<P>(
    ctx: &RenderCtx<'_>,
    spec: &ServiceSpec<P::Config>,
    replicas: u32,
) -> Result<Vec<Value>, InstanceRenderError>
where
    P: ServiceProfile,
    P::Config: Serialize + Clone,
{
    if replicas == 0 {
        return Err(InstanceRenderError::NoReplicas);
    }
    let mut copy = spec.clone();
    copy.replicas.replicas_per_shard = replicas;
    copy.replicas.voters = replicas.min(spec.replicas.voters);
    render_instance::<P>(ctx, &copy, 1)
}

#[cfg(test)]
mod tests;
