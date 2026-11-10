//! One scheduled backup Job: the `batch/v1` Job the caller creates for one
//! schedule slot. The planner in `domain::backup_job` decides when.

use serde_json::{json, Value};

use super::{PodPlan, RenderCtx};
use crate::domain::backup_job::BackupJobPolicy;

#[derive(Clone, Debug)]
pub struct BackupJobPlan {
    name: String,
    policy: BackupJobPolicy,
    pod: PodPlan,
}

impl BackupJobPlan {
    pub fn new(name: impl Into<String>, policy: BackupJobPolicy, pod: PodPlan) -> Self {
        Self {
            name: name.into(),
            policy,
            pod,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render(mut self, cx: &RenderCtx<'_>) -> Value {
        self.pod.runtime.restart_policy = Some("Never".into());
        let component = self.pod.component.clone();
        let policy = self.policy;
        json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": cx.meta(&self.name, &component),
            "spec": {
                "backoffLimit": policy.backoff_limit,
                "activeDeadlineSeconds": policy.active_deadline_seconds,
                "ttlSecondsAfterFinished": policy.ttl_seconds_after_finished,
                "template": self.pod.render(cx),
            },
        })
    }
}
