//! Workload controller plans: PVCs, StatefulSets, Deployments, DaemonSets, PDBs
//! and CronJobs.

use serde_json::{json, Value};

use super::{merge_string_labels, LabelSet, PodPlan, RenderCtx, WorkloadPlanError};

#[derive(Clone, Debug)]
pub struct PersistentVolumeClaimPlan {
    pub name: String,
    pub component: String,
    pub storage: String,
    pub mount_path: String,
    pub access_modes: Vec<String>,
}

impl PersistentVolumeClaimPlan {
    pub fn new(
        name: impl Into<String>,
        component: impl Into<String>,
        storage: impl Into<String>,
        mount_path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            storage: storage.into(),
            mount_path: mount_path.into(),
            access_modes: vec!["ReadWriteOnce".into()],
        }
    }

    fn standalone(&self, cx: &RenderCtx<'_>) -> Value {
        json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": cx.meta(&self.name, &self.component),
            "spec": {
                "accessModes": self.access_modes,
                "resources": {"requests": {"storage": self.storage}},
            },
        })
    }

    fn template(&self) -> Value {
        json!({
            "metadata": {"name": self.name},
            "spec": {
                "accessModes": self.access_modes,
                "resources": {"requests": {"storage": self.storage}},
            },
        })
    }
}

fn attach_claim(template: &mut Value, volume_name: &str, claim_name: Option<&str>, mount: &str) {
    let pod = template["spec"]
        .as_object_mut()
        .expect("typed pod spec is an object");
    let containers = pod["containers"]
        .as_array_mut()
        .expect("typed pod containers are an array");
    let mounts = containers[0]["volumeMounts"]
        .as_array_mut()
        .expect("typed volume mounts are an array");
    mounts.insert(0, json!({"name": volume_name, "mountPath": mount}));
    if let Some(claim_name) = claim_name {
        let volumes = pod
            .entry("volumes")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("typed pod volumes are an array");
        volumes.insert(
            0,
            json!({
                "name": volume_name,
                "persistentVolumeClaim": {"claimName": claim_name},
            }),
        );
    }
}

#[derive(Clone, Debug)]
pub struct StatefulSetPlan {
    pub name: String,
    pub service_name: String,
    pub replicas: u32,
    pub pod_management_policy: String,
    pub pod: PodPlan,
    pub claim: PersistentVolumeClaimPlan,
}

impl StatefulSetPlan {
    pub fn new(
        name: impl Into<String>,
        service_name: impl Into<String>,
        replicas: u32,
        pod: PodPlan,
        claim: PersistentVolumeClaimPlan,
    ) -> Self {
        Self {
            name: name.into(),
            service_name: service_name.into(),
            replicas,
            pod_management_policy: "Parallel".into(),
            pod,
            claim,
        }
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Result<Value, WorkloadPlanError> {
        if self.replicas == 0 {
            return Err(WorkloadPlanError::ZeroReplicas(self.name));
        }
        let selector = self.pod.selector(cx);
        let component = self.pod.component.clone();
        let mut template = self.pod.render(cx);
        attach_claim(
            &mut template,
            &self.claim.name,
            None,
            &self.claim.mount_path,
        );
        Ok(json!({
            "apiVersion": "apps/v1",
            "kind": "StatefulSet",
            "metadata": cx.meta(&self.name, &component),
            "spec": {
                "serviceName": self.service_name,
                "replicas": self.replicas,
                "podManagementPolicy": self.pod_management_policy,
                "selector": {"matchLabels": selector},
                "template": template,
                "volumeClaimTemplates": [self.claim.template()],
            },
        }))
    }
}

#[derive(Clone, Debug)]
pub struct DeploymentPlan {
    pub name: String,
    pub replicas: u32,
    pub strategy: Option<Value>,
    pub pod: PodPlan,
    pub claim: Option<PersistentVolumeClaimPlan>,
}

impl DeploymentPlan {
    pub fn new(name: impl Into<String>, replicas: u32, pod: PodPlan) -> Self {
        Self {
            name: name.into(),
            replicas,
            strategy: None,
            pod,
            claim: None,
        }
    }

    pub fn with_persistent_claim(mut self, claim: PersistentVolumeClaimPlan) -> Self {
        self.claim = Some(claim);
        self
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Result<Vec<Value>, WorkloadPlanError> {
        if self.replicas == 0 {
            return Err(WorkloadPlanError::ZeroReplicas(self.name));
        }
        let selector = self.pod.selector(cx);
        let component = self.pod.component.clone();
        let mut template = self.pod.render(cx);
        let mut objects = Vec::new();
        if let Some(claim) = self.claim {
            objects.push(claim.standalone(cx));
            attach_claim(&mut template, "data", Some(&claim.name), &claim.mount_path);
        }
        let mut spec = json!({
            "replicas": self.replicas,
            "selector": {"matchLabels": selector},
            "template": template,
        });
        if let Some(strategy) = self.strategy {
            spec["strategy"] = strategy;
        }
        objects.push(json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": cx.meta(&self.name, &component),
            "spec": spec,
        }));
        Ok(objects)
    }
}

#[derive(Clone, Debug)]
pub struct DaemonSetPlan {
    pub name: String,
    pub pod: PodPlan,
}

impl DaemonSetPlan {
    pub fn new(name: impl Into<String>, pod: PodPlan) -> Self {
        Self {
            name: name.into(),
            pod,
        }
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let selector = self.pod.selector(cx);
        let component = self.pod.component.clone();
        json!({
            "apiVersion": "apps/v1",
            "kind": "DaemonSet",
            "metadata": cx.meta(&self.name, &component),
            "spec": {
                "selector": {"matchLabels": selector},
                "template": self.pod.render(cx),
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct PodDisruptionBudgetPlan {
    pub name: String,
    pub component: String,
    pub selector: LabelSet,
    pub min_available: u32,
}

impl PodDisruptionBudgetPlan {
    pub fn min_available(
        name: impl Into<String>,
        component: impl Into<String>,
        selector: LabelSet,
        min_available: u32,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            selector,
            min_available,
        }
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let selector = merge_string_labels(cx.selector(&self.component), &self.selector);
        json!({
            "apiVersion": "policy/v1",
            "kind": "PodDisruptionBudget",
            "metadata": cx.meta(&self.name, &self.component),
            "spec": {"minAvailable": self.min_available, "selector": {"matchLabels": selector}},
        })
    }
}

/// A CronJob that runs `pod` on `schedule`, one run at a time.
#[derive(Clone, Debug)]
pub struct CronJobPlan {
    name: String,
    schedule: String,
    successful_jobs_history_limit: i32,
    failed_jobs_history_limit: i32,
    pod: PodPlan,
    suspend: Option<bool>,
    starting_deadline_seconds: Option<i64>,
    active_deadline_seconds: Option<i64>,
    backoff_limit: Option<i32>,
    ttl_seconds_after_finished: Option<i32>,
}

impl CronJobPlan {
    /// A CronJob that keeps Kubernetes' default history: 3 successful and
    /// 1 failed Job. The builders below change either limit.
    pub fn new(name: impl Into<String>, schedule: impl Into<String>, pod: PodPlan) -> Self {
        Self {
            name: name.into(),
            schedule: schedule.into(),
            successful_jobs_history_limit: 3,
            failed_jobs_history_limit: 1,
            pod,
            suspend: None,
            starting_deadline_seconds: None,
            active_deadline_seconds: None,
            backoff_limit: None,
            ttl_seconds_after_finished: None,
        }
    }

    pub fn with_successful_jobs_history_limit(mut self, limit: i32) -> Self {
        self.successful_jobs_history_limit = limit;
        self
    }

    pub fn with_failed_jobs_history_limit(mut self, limit: i32) -> Self {
        self.failed_jobs_history_limit = limit;
        self
    }

    pub fn with_suspend(mut self, suspend: bool) -> Self {
        self.suspend = Some(suspend);
        self
    }

    pub fn with_starting_deadline_seconds(mut self, seconds: i64) -> Self {
        self.starting_deadline_seconds = Some(seconds);
        self
    }

    pub fn with_active_deadline_seconds(mut self, seconds: i64) -> Self {
        self.active_deadline_seconds = Some(seconds);
        self
    }

    pub fn with_backoff_limit(mut self, limit: i32) -> Self {
        self.backoff_limit = Some(limit);
        self
    }

    pub fn with_ttl_seconds_after_finished(mut self, seconds: i32) -> Self {
        self.ttl_seconds_after_finished = Some(seconds);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schedule(&self) -> &str {
        &self.schedule
    }

    pub fn successful_jobs_history_limit(&self) -> i32 {
        self.successful_jobs_history_limit
    }

    pub fn failed_jobs_history_limit(&self) -> i32 {
        self.failed_jobs_history_limit
    }

    pub fn pod(&self) -> &PodPlan {
        &self.pod
    }

    pub fn suspend(&self) -> Option<bool> {
        self.suspend
    }

    pub fn starting_deadline_seconds(&self) -> Option<i64> {
        self.starting_deadline_seconds
    }

    pub fn active_deadline_seconds(&self) -> Option<i64> {
        self.active_deadline_seconds
    }

    pub fn backoff_limit(&self) -> Option<i32> {
        self.backoff_limit
    }

    pub fn ttl_seconds_after_finished(&self) -> Option<i32> {
        self.ttl_seconds_after_finished
    }

    pub(super) fn render(mut self, cx: &RenderCtx<'_>) -> Result<Value, WorkloadPlanError> {
        if let Some(s) = self.starting_deadline_seconds {
            if s < 0 {
                return Err(WorkloadPlanError::NegativeValue(
                    self.name,
                    "startingDeadlineSeconds",
                ));
            }
        }
        if let Some(s) = self.active_deadline_seconds {
            if s < 0 {
                return Err(WorkloadPlanError::NegativeValue(
                    self.name,
                    "activeDeadlineSeconds",
                ));
            }
        }
        if let Some(l) = self.backoff_limit {
            if l < 0 {
                return Err(WorkloadPlanError::NegativeValue(self.name, "backoffLimit"));
            }
        }
        if let Some(s) = self.ttl_seconds_after_finished {
            if s < 0 {
                return Err(WorkloadPlanError::NegativeValue(
                    self.name,
                    "ttlSecondsAfterFinished",
                ));
            }
        }

        self.pod.runtime.restart_policy = Some("OnFailure".into());
        let component = self.pod.component.clone();
        let mut job_template_spec = json!({
            "template": self.pod.render(cx),
        });
        if let Some(active_deadline) = self.active_deadline_seconds {
            job_template_spec["activeDeadlineSeconds"] = json!(active_deadline);
        }
        if let Some(backoff) = self.backoff_limit {
            job_template_spec["backoffLimit"] = json!(backoff);
        }
        if let Some(ttl) = self.ttl_seconds_after_finished {
            job_template_spec["ttlSecondsAfterFinished"] = json!(ttl);
        }

        let mut spec = json!({
            "schedule": self.schedule,
            "concurrencyPolicy": "Forbid",
            "successfulJobsHistoryLimit": self.successful_jobs_history_limit,
            "failedJobsHistoryLimit": self.failed_jobs_history_limit,
            "jobTemplate": {"spec": job_template_spec},
        });
        if let Some(suspend) = self.suspend {
            spec["suspend"] = json!(suspend);
        }
        if let Some(starting_deadline) = self.starting_deadline_seconds {
            spec["startingDeadlineSeconds"] = json!(starting_deadline);
        }

        Ok(json!({
            "apiVersion": "batch/v1",
            "kind": "CronJob",
            "metadata": cx.meta(&self.name, &component),
            "spec": spec,
        }))
    }
}
