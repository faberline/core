//! Workload-neutral Pod templates and ordinary Kubernetes child helpers.
//!
//! The helpers are re-exported from the existing render root during the first
//! service-k8s landing so StatefulSet consumers keep their source-compatible
//! imports. New workload profiles compose through this semantic module.

use serde_json::{json, Value};

use crate::domain::lifecycle::{
    TerminationBudget, DRAIN_ENDPOINT_PATH, ENV_SERVICE_RUNTIME_DEADLINE_SECONDS,
    ENV_SERVICE_SIGKILL_RESERVE_SECONDS,
};

pub use super::{
    client_service, client_service_with_ports, cron_job, guaranteed_resources,
    horizontal_pod_autoscaler, owner_ref, pdb, requested_resources, service_account, CronJob,
    HorizontalPodAutoscaler, RenderCtx,
};

/// Workload-neutral Pod contract used by the Deployment profile and available
/// to other workload renderers. It deliberately contains no stable identity,
/// PVC, shard, ordinal, peer, or session-affinity fields.
pub struct ServicePodTemplate<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub component: &'a str,
    pub image: &'a str,
    pub image_pull_policy: &'a str,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub ports: Vec<Value>,
    pub env: Vec<Value>,
    pub env_from: Vec<Value>,
    pub resources: Value,
    pub readiness_probe: Option<Value>,
    pub liveness_probe: Option<Value>,
    pub startup_probe: Option<Value>,
    pub lifecycle: Option<Value>,
    pub container_security_context: Option<Value>,
    pub pod_security_context: Option<Value>,
    pub service_account_name: Option<&'a str>,
    pub termination_grace_period_seconds: Option<u64>,
    pub volumes: Vec<Value>,
    pub volume_mounts: Vec<Value>,
    pub pod_annotations: Option<Value>,
    pub topology_spread_constraints: Vec<Value>,
}

/// Helper to apply a validated [`TerminationBudget`]'s termination contract (grace period, preStop hook, and runtime env entries) without mutating probes.
pub fn apply_termination_contract(
    budget: &TerminationBudget,
    probe_port: u16,
    env: &mut Vec<Value>,
    termination_grace_period_seconds: &mut Option<u64>,
    lifecycle: &mut Option<Value>,
) {
    *termination_grace_period_seconds = Some(budget.total_grace_period_seconds());

    if budget.prestop_cost_seconds().is_some() {
        *lifecycle = Some(json!({
            "preStop": {
                "httpGet": {
                    "path": DRAIN_ENDPOINT_PATH,
                    "port": probe_port,
                }
            }
        }));
    }

    let deadline_val = budget.runtime_deadline_seconds().to_string();
    let reserve_val = budget.sigkill_reserve_seconds().to_string();

    let mut deadline_found = false;
    let mut reserve_found = false;

    let mut new_env = Vec::with_capacity(env.len() + 2);
    for item in env.drain(..) {
        let name = item.get("name").and_then(|n| n.as_str());
        if name == Some(ENV_SERVICE_RUNTIME_DEADLINE_SECONDS) {
            if !deadline_found {
                deadline_found = true;
                let mut updated = item.clone();
                updated["value"] = json!(deadline_val);
                new_env.push(updated);
            }
        } else if name == Some(ENV_SERVICE_SIGKILL_RESERVE_SECONDS) {
            if !reserve_found {
                reserve_found = true;
                let mut updated = item.clone();
                updated["value"] = json!(reserve_val);
                new_env.push(updated);
            }
        } else {
            new_env.push(item);
        }
    }

    if !deadline_found {
        new_env.push(json!({
            "name": ENV_SERVICE_RUNTIME_DEADLINE_SECONDS,
            "value": deadline_val,
        }));
    }
    if !reserve_found {
        new_env.push(json!({
            "name": ENV_SERVICE_SIGKILL_RESERVE_SECONDS,
            "value": reserve_val,
        }));
    }

    *env = new_env;
}

/// Helper to apply a validated [`TerminationBudget`] to pod/container fields.
pub fn apply_termination_budget(
    budget: &TerminationBudget,
    probe_port: u16,
    env: &mut Vec<Value>,
    liveness_probe: &mut Option<Value>,
    readiness_probe: &mut Option<Value>,
    startup_probe: &mut Option<Value>,
    termination_grace_period_seconds: &mut Option<u64>,
    lifecycle: &mut Option<Value>,
) {
    *liveness_probe = Some(budget.render_liveness_probe(probe_port));
    *readiness_probe = Some(budget.render_readiness_probe(probe_port));
    *startup_probe = Some(budget.render_startup_probe(probe_port));
    apply_termination_contract(
        budget,
        probe_port,
        env,
        termination_grace_period_seconds,
        lifecycle,
    );
}

impl ServicePodTemplate<'_> {
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

    /// Render the `spec.template` value shared by workload controllers.
    pub fn render(self) -> Value {
        let mut container = json!({
            "name": self.component,
            "image": self.image,
            "imagePullPolicy": self.image_pull_policy,
            "command": self.command,
            "ports": self.ports,
            "env": self.env,
            "resources": self.resources,
        });
        if !self.args.is_empty() {
            container["args"] = json!(self.args);
        }
        if !self.env_from.is_empty() {
            container["envFrom"] = json!(self.env_from);
        }
        if let Some(probe) = self.readiness_probe {
            container["readinessProbe"] = probe;
        }
        if let Some(probe) = self.liveness_probe {
            container["livenessProbe"] = probe;
        }
        if let Some(probe) = self.startup_probe {
            container["startupProbe"] = probe;
        }
        if let Some(lifecycle) = self.lifecycle {
            container["lifecycle"] = lifecycle;
        }
        if let Some(context) = self.container_security_context {
            container["securityContext"] = context;
        }
        if !self.volume_mounts.is_empty() {
            container["volumeMounts"] = json!(self.volume_mounts);
        }

        let mut pod_spec = json!({ "containers": [container] });
        if let Some(name) = self.service_account_name {
            pod_spec["serviceAccountName"] = json!(name);
        }
        if let Some(seconds) = self.termination_grace_period_seconds {
            pod_spec["terminationGracePeriodSeconds"] = json!(seconds);
        }
        if let Some(context) = self.pod_security_context {
            pod_spec["securityContext"] = context;
        }
        if !self.volumes.is_empty() {
            pod_spec["volumes"] = json!(self.volumes);
        }
        if !self.topology_spread_constraints.is_empty() {
            pod_spec["topologySpreadConstraints"] = json!(self.topology_spread_constraints);
        }

        let mut metadata = json!({ "labels": self.cx.labels(self.component) });
        if let Some(annotations) = self.pod_annotations {
            metadata["annotations"] = annotations;
        }
        json!({ "metadata": metadata, "spec": pod_spec })
    }
}

/// One instance's default-deny network posture, expressed as the two peer
/// classes a sharded service actually has (#2603).
///
/// A NetworkPolicy is deny-by-default *for the pods it selects*: once any
/// policy selects a pod, only the union of matching rules is permitted. That
/// makes the two port lists the whole contract — `client_ports` is the public
/// API surface, `peer_ports` is consensus/replication traffic that must never
/// be reachable from outside the instance.
pub struct NetworkPolicy<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    /// Ports any workload in the cluster may reach — the service's client API.
    /// Empty means the instance accepts no ingress at all.
    pub client_ports: Vec<i32>,
    /// Ports only this instance's own pods may reach, in both directions —
    /// Raft, replication, gossip. Empty for a stateless single-pod service.
    pub peer_ports: Vec<i32>,
    /// Egress rules beyond the DNS + TLS baseline, as raw
    /// `networking.k8s.io/v1` egress entries. A service that talks to a
    /// non-443 external dependency (a broker, a database) supplies it here;
    /// most services leave this empty.
    pub extra_egress: Vec<Value>,
}

fn tcp_ports(ports: &[i32]) -> Vec<Value> {
    ports
        .iter()
        .map(|port| json!({ "protocol": "TCP", "port": port }))
        .collect()
}

/// Render the instance's NetworkPolicy.
///
/// The peer rules select on [`RenderCtx::selector`], which includes
/// `app.kubernetes.io/instance` — so two Lumen CRs sharing a namespace cannot
/// reach each other's consensus ports, only their own siblings'.
pub fn network_policy(p: NetworkPolicy<'_>) -> Value {
    let selector = p.cx.selector(p.component);
    let peers = json!({ "podSelector": { "matchLabels": selector } });

    let mut ingress = Vec::new();
    if !p.client_ports.is_empty() {
        // `namespaceSelector: {}` is every namespace, not every source: it
        // still excludes anything outside the pod network (a LoadBalancer's
        // external client reaches the pod through a node, which this rule does
        // not admit). Cluster-internal reach is the intended API posture.
        ingress.push(json!({
            "from": [{ "namespaceSelector": {} }],
            "ports": tcp_ports(&p.client_ports),
        }));
    }
    if !p.peer_ports.is_empty() {
        ingress.push(json!({ "from": [peers], "ports": tcp_ports(&p.peer_ports) }));
    }

    let mut egress = Vec::new();
    if !p.peer_ports.is_empty() {
        egress.push(json!({ "to": [peers], "ports": tcp_ports(&p.peer_ports) }));
    }
    // DNS (both transports — a truncated UDP answer retries over TCP) plus
    // outbound TLS, which is what object-storage backups, OIDC discovery, and
    // image-independent HTTPS calls need. Plaintext :80 is deliberately not
    // granted; a service that needs it declares `extra_egress`.
    egress.push(json!({
        "ports": [
            { "protocol": "UDP", "port": 53 },
            { "protocol": "TCP", "port": 53 },
            { "protocol": "TCP", "port": 443 },
        ],
    }));
    egress.extend(p.extra_egress);

    json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": p.cx.meta(p.name, p.component),
        "spec": {
            "podSelector": { "matchLabels": p.cx.selector(p.component) },
            "policyTypes": ["Ingress", "Egress"],
            "ingress": ingress,
            "egress": egress,
        },
    })
}

#[cfg(test)]
mod tests;
