//! Parameter structs and renderers for the HPA, the maintenance CronJob and the
//! sharded StatefulSet.

use serde_json::{json, Value};

use super::common::{apply_termination_budget, apply_termination_contract};
use super::{dedicated_node_affinity, requested_resources, RenderCtx, WorkloadVolumeClaim};
use crate::domain::lifecycle::TerminationBudget;

/// Parameters for [`horizontal_pod_autoscaler`].
pub struct HorizontalPodAutoscaler<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    pub target_api_version: &'a str,
    pub target_kind: &'a str,
    pub target_name: &'a str,
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub metrics: Vec<Value>,
    pub behavior: Option<Value>,
}

/// A HorizontalPodAutoscaler targeting a rendered service workload.
pub fn horizontal_pod_autoscaler(p: HorizontalPodAutoscaler) -> Value {
    let HorizontalPodAutoscaler {
        cx,
        name,
        component,
        target_api_version,
        target_kind,
        target_name,
        min_replicas,
        max_replicas,
        metrics,
        behavior,
    } = p;
    let mut spec = json!({
        "scaleTargetRef": {
            "apiVersion": target_api_version,
            "kind": target_kind,
            "name": target_name,
        },
        "minReplicas": min_replicas,
        "maxReplicas": max_replicas,
        "metrics": metrics,
    });
    if let Some(behavior) = behavior {
        spec["behavior"] = behavior;
    }
    json!({
        "apiVersion": "autoscaling/v2",
        "kind": "HorizontalPodAutoscaler",
        "metadata": cx.meta(name, component),
        "spec": spec,
    })
}

/// Parameters for [`cron_job`].
pub struct CronJob<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    pub schedule: &'a str,
    pub image: &'a str,
    pub image_pull_policy: &'a str,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub env: Vec<Value>,
    pub env_from: Vec<Value>,
    pub volumes: Vec<Value>,
    pub volume_mounts: Vec<Value>,
    pub service_account_name: Option<&'a str>,
    pub cpu: &'a str,
    pub memory: &'a str,
    pub successful_jobs_history_limit: i32,
    pub failed_jobs_history_limit: i32,
}

/// A CronJob for service-side maintenance runners such as object-store backups.
///
/// Operators schedule and wire the runner; the service or runner still owns the
/// actual domain bytes. This helper deliberately stays manifest-only.
pub fn cron_job(p: CronJob) -> Value {
    let cx = p.cx;
    let mut container = json!({
        "name": p.component,
        "image": p.image,
        "imagePullPolicy": p.image_pull_policy,
        "command": p.command,
        "args": p.args,
        "env": p.env,
        "resources": {
            "requests": { "cpu": p.cpu, "memory": p.memory },
            "limits": { "cpu": p.cpu, "memory": p.memory },
        },
    });
    if !p.env_from.is_empty() {
        container["envFrom"] = json!(p.env_from);
    }
    if !p.volume_mounts.is_empty() {
        container["volumeMounts"] = json!(p.volume_mounts);
    }

    let mut pod_spec = json!({
        "restartPolicy": "OnFailure",
        "containers": [container],
    });
    if let Some(service_account_name) = p.service_account_name {
        pod_spec["serviceAccountName"] = json!(service_account_name);
    }
    if !p.volumes.is_empty() {
        pod_spec["volumes"] = json!(p.volumes);
    }

    json!({
        "apiVersion": "batch/v1",
        "kind": "CronJob",
        "metadata": cx.meta(p.name, p.component),
        "spec": {
            "schedule": p.schedule,
            "concurrencyPolicy": "Forbid",
            "successfulJobsHistoryLimit": p.successful_jobs_history_limit,
            "failedJobsHistoryLimit": p.failed_jobs_history_limit,
            "jobTemplate": {
                "spec": {
                    "template": {
                        "metadata": { "labels": cx.labels(p.component) },
                        "spec": pod_spec,
                    },
                },
            },
        },
    })
}

/// Parameters for [`service_statefulset`].
pub struct ServiceStatefulSet<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    pub image: &'a str,
    pub image_pull_policy: &'a str,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub ports: Vec<Value>,
    /// The headless Service name (`serviceName`) + the value of `headless_env_key`.
    pub headless_service: &'a str,
    pub shard_count: u32,
    pub replicas_per_shard: u32,
    pub voter_count: u32,
    /// The env key the service reads for its headless-DNS suffix
    /// (e.g. `LUMEN_HEADLESS_SERVICE`).
    pub headless_env_key: &'a str,
    pub service_account_name: Option<&'a str>,
    pub env: Vec<Value>,
    pub env_from: Vec<Value>,
    pub resources: Value,
    pub pod_annotations: Option<Value>,
    pub pod_security_context: Option<Value>,
    pub container_security_context: Option<Value>,
    pub termination_grace_period_seconds: Option<u64>,
    pub readiness_probe: Option<Value>,
    pub liveness_probe: Option<Value>,
    pub startup_probe: Option<Value>,
    pub lifecycle: Option<Value>,
    pub volumes: Vec<Value>,
    pub volume_mounts: Vec<Value>,
    /// Pod affinity/anti-affinity. Stateful data-plane callers should use
    /// [`dedicated_node_affinity`].
    pub affinity: Option<Value>,
    /// `spec.template.spec.nodeSelector` — which node pool the workload runs
    /// on. A field of its own rather than something a caller expresses through
    /// [`Self::affinity`]: naming a node pool must not require restating the
    /// operator-owned pod anti-affinity, because a caller that restates it
    /// wrongly loses the constraint keeping two replicas of one shard off the
    /// same host, and the resulting StatefulSet looks correct.
    pub node_selector: Option<Value>,
    /// `spec.template.spec.tolerations` — the taints this workload may land
    /// on, so a dedicated node pool can repel every other workload.
    pub tolerations: Vec<Value>,
    pub topology_spread_constraints: Vec<Value>,
    pub revision_history_limit: Option<i32>,
    pub update_strategy: Option<Value>,
    /// `Some(pvc)` for a durable workload (adds the claim template + mount).
    pub volume_claim: Option<WorkloadVolumeClaim<'a>>,
}

impl ServiceStatefulSet<'_> {
    /// Apply probes, preStop lifecycle hook, environment variables, and termination grace period derived from a validated [`TerminationBudget`].
    pub fn with_termination_budget(mut self, budget: &TerminationBudget, probe_port: u16) -> Self {
        apply_termination_budget(
            budget,
            probe_port,
            &mut self.env,
            &mut self.liveness_probe,
            &mut self.readiness_probe,
            &mut self.startup_probe,
            &mut self.termination_grace_period_seconds,
            &mut self.lifecycle,
        );
        self
    }

    /// Apply termination grace period, preStop lifecycle hook, and environment variables derived from a validated [`TerminationBudget`], without mutating probes.
    pub fn with_termination_contract(
        mut self,
        budget: &TerminationBudget,
        probe_port: u16,
    ) -> Self {
        apply_termination_contract(
            budget,
            probe_port,
            &mut self.env,
            &mut self.termination_grace_period_seconds,
            &mut self.lifecycle,
        );
        self
    }
}

/// A configurable, downward-API StatefulSet primitive for sharded service
/// workloads. It preserves the exact raft-runtime env contract while letting a
/// service supply its own probes, security hardening, storage path, extra
/// volumes, and rollout details.
pub fn service_statefulset(p: ServiceStatefulSet) -> Value {
    crate::render::stateful_instance::render_compat_service_statefulset(p)
}

/// Render a StatefulSet with an explicit Kubernetes service-link setting.
/// This additive entry point keeps the historical struct and renderer
/// source-compatible.
pub fn service_statefulset_with_service_links(
    p: ServiceStatefulSet,
    enable_service_links: bool,
) -> Value {
    super::stateful_instance::render_compat_service_statefulset_with_links(p, enable_service_links)
}

/// Parameters for [`sharded_statefulset`].
pub struct ShardedStatefulSet<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    pub image: &'a str,
    pub image_pull_policy: &'a str,
    pub command: Vec<String>,
    pub ports: Vec<(&'a str, i32)>,
    /// The headless Service name (`serviceName`) + the value of `headless_env_key`.
    pub headless_service: &'a str,
    pub shard_count: u32,
    pub replicas_per_shard: u32,
    pub voter_count: u32,
    /// The env key the service reads for its headless-DNS suffix
    /// (e.g. `LUMEN_HEADLESS_SERVICE`).
    pub headless_env_key: &'a str,
    pub cpu: &'a str,
    pub memory: &'a str,
    /// Service-specific env appended after the downward-API quartet.
    pub extra_env: Vec<Value>,
    /// `Some(pvc)` for a durable workload (adds the claim template + a `/data` mount).
    pub volume_claim: Option<Value>,
}

/// The downward-API StatefulSet: `replicas = shard_count * replicas_per_shard`,
/// `podManagementPolicy: Parallel`, and the env quartet
/// (`POD_NAME`/`POD_NAMESPACE`/`SHARD_COUNT`/`REPLICAS_PER_SHARD`/`VOTER_COUNT`)
/// together with `<headless_env_key>`, which `raft_runtime::ClusterTopology::from_env`
/// reads to derive node id / membership / peers.
pub fn sharded_statefulset(p: ShardedStatefulSet) -> Value {
    let volume_claim = p.volume_claim.map(|template| {
        let name = template["metadata"]["name"]
            .as_str()
            .unwrap_or("data")
            .to_owned();
        WorkloadVolumeClaim {
            name,
            template,
            mount_path: "/data",
            read_only: false,
        }
    });

    service_statefulset(ServiceStatefulSet {
        cx: p.cx,
        name: p.name,
        component: p.component,
        image: p.image,
        image_pull_policy: p.image_pull_policy,
        command: p.command,
        args: vec![],
        ports: p
            .ports
            .iter()
            .map(|(n, port)| json!({ "name": n, "containerPort": port, "protocol": "TCP" }))
            .collect(),
        headless_service: p.headless_service,
        shard_count: p.shard_count,
        replicas_per_shard: p.replicas_per_shard,
        voter_count: p.voter_count,
        headless_env_key: p.headless_env_key,
        service_account_name: Some(p.cx.name),
        env: p.extra_env,
        env_from: vec![],
        resources: requested_resources(p.cpu, p.memory),
        pod_annotations: None,
        pod_security_context: None,
        container_security_context: None,
        termination_grace_period_seconds: None,
        readiness_probe: None,
        liveness_probe: None,
        startup_probe: None,
        lifecycle: None,
        volumes: vec![],
        volume_mounts: vec![],
        affinity: Some(dedicated_node_affinity(p.cx.selector(p.component))),
        node_selector: None,
        tolerations: vec![],
        topology_spread_constraints: vec![],
        revision_history_limit: None,
        update_strategy: None,
        volume_claim,
    })
}
