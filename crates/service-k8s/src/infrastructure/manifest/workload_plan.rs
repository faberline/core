//! Typed composition for a complete Kubernetes workload set.
//!
//! A product supplies names, selectors, ports, storage sizes, and security
//! policy. This module owns the repeated Kubernetes object shapes.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use thiserror::Error;

use super::RenderCtx;

mod access;
mod network;
mod pod;
mod service;
mod workload;

pub use access::{
    ClusterRoleBindingPlan, RbacRulePlan, RoleBindingPlan, RolePlan, ServiceAccountSubjectPlan,
};
pub use network::{
    FqdnMatchPlan, FqdnNetworkPolicyPlan, NetworkPeerPlan, NetworkPolicyPlan, NetworkPortPlan,
    NetworkRulePlan,
};
pub use pod::{ContainerPlan, PodPlan, PodRuntimePolicy};
pub use service::{ServiceAccountPlan, ServicePlan, ServicePortPlan};
pub use workload::{
    CronJobPlan, DaemonSetPlan, DeploymentPlan, PersistentVolumeClaimPlan, PodDisruptionBudgetPlan,
    StatefulSetPlan,
};

pub type LabelSet = BTreeMap<String, String>;

fn merge_string_labels(mut base: Value, extra: &LabelSet) -> Value {
    let object = base.as_object_mut().expect("render labels are an object");
    for (key, value) in extra {
        object.insert(key.clone(), json!(value));
    }
    base
}

enum PlannedObject {
    ServiceAccount(ServiceAccountPlan),
    Service(ServicePlan),
    StatefulSet(StatefulSetPlan),
    Deployment(DeploymentPlan),
    DaemonSet(DaemonSetPlan),
    PodDisruptionBudget(PodDisruptionBudgetPlan),
    Role(RolePlan),
    RoleBinding(RoleBindingPlan),
    ClusterRoleBinding(ClusterRoleBindingPlan),
    CronJob(CronJobPlan),
    NetworkPolicy(NetworkPolicyPlan),
    FqdnNetworkPolicy(FqdnNetworkPolicyPlan),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkloadPlanError {
    #[error("workload {0} must have at least one replica")]
    ZeroReplicas(String),
}

/// One ordered set of typed Kubernetes children for a managed service.
pub struct WorkloadPlan<'a> {
    cx: &'a RenderCtx<'a>,
    objects: Vec<PlannedObject>,
}

impl<'a> WorkloadPlan<'a> {
    pub fn new(cx: &'a RenderCtx<'a>) -> Self {
        Self {
            cx,
            objects: Vec::new(),
        }
    }

    pub fn add_service_account(&mut self, plan: ServiceAccountPlan) {
        self.objects.push(PlannedObject::ServiceAccount(plan));
    }

    pub fn add_service(&mut self, plan: ServicePlan) {
        self.objects.push(PlannedObject::Service(plan));
    }

    pub fn add_stateful_set(&mut self, plan: StatefulSetPlan) {
        self.objects.push(PlannedObject::StatefulSet(plan));
    }

    pub fn add_deployment(&mut self, plan: DeploymentPlan) {
        self.objects.push(PlannedObject::Deployment(plan));
    }

    pub fn add_daemon_set(&mut self, plan: DaemonSetPlan) {
        self.objects.push(PlannedObject::DaemonSet(plan));
    }

    pub fn add_pod_disruption_budget(&mut self, plan: PodDisruptionBudgetPlan) {
        self.objects.push(PlannedObject::PodDisruptionBudget(plan));
    }

    pub fn add_role(&mut self, plan: RolePlan) {
        self.objects.push(PlannedObject::Role(plan));
    }

    pub fn add_role_binding(&mut self, plan: RoleBindingPlan) {
        self.objects.push(PlannedObject::RoleBinding(plan));
    }

    pub fn add_cluster_role_binding(&mut self, plan: ClusterRoleBindingPlan) {
        self.objects.push(PlannedObject::ClusterRoleBinding(plan));
    }

    pub fn add_cron_job(&mut self, plan: CronJobPlan) {
        self.objects.push(PlannedObject::CronJob(plan));
    }

    pub fn add_network_policy(&mut self, plan: NetworkPolicyPlan) {
        self.objects.push(PlannedObject::NetworkPolicy(plan));
    }

    pub fn add_fqdn_network_policy(&mut self, plan: FqdnNetworkPolicyPlan) {
        self.objects.push(PlannedObject::FqdnNetworkPolicy(plan));
    }

    pub fn render(self) -> Result<Vec<Value>, WorkloadPlanError> {
        let mut rendered = Vec::new();
        for object in self.objects {
            match object {
                PlannedObject::ServiceAccount(plan) => rendered.push(json!({
                    "apiVersion": "v1",
                    "kind": "ServiceAccount",
                    "metadata": self.cx.meta(&plan.name, &plan.component),
                    "automountServiceAccountToken": plan.automount_service_account_token,
                })),
                PlannedObject::Service(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::StatefulSet(plan) => rendered.push(plan.render(self.cx)?),
                PlannedObject::Deployment(plan) => rendered.extend(plan.render(self.cx)?),
                PlannedObject::DaemonSet(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::PodDisruptionBudget(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::Role(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::RoleBinding(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::ClusterRoleBinding(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::CronJob(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::NetworkPolicy(plan) => rendered.push(plan.render(self.cx)),
                PlannedObject::FqdnNetworkPolicy(plan) => rendered.push(plan.render(self.cx)),
            }
        }
        Ok(rendered)
    }
}
