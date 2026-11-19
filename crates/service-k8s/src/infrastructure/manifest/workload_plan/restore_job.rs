//! One restore Job: the `batch/v1` Job the caller creates for one restore
//! request. The planner in `domain::restore_job` decides when.

use serde_json::{json, Value};

use super::{PodPlan, RenderCtx};
use crate::domain::backup_status::BackupManifestRef;
use crate::domain::restore_job::{RestoreJobPolicy, RESTORE_FINALIZER};

#[derive(Clone, Debug)]
pub struct RestoreJobPlan {
    name: String,
    policy: RestoreJobPolicy,
    manifest: BackupManifestRef,
    pod: PodPlan,
}

impl RestoreJobPlan {
    pub fn new(
        name: impl Into<String>,
        policy: RestoreJobPolicy,
        manifest: BackupManifestRef,
        pod: PodPlan,
    ) -> Self {
        Self {
            name: name.into(),
            policy,
            manifest,
            pod,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render(mut self, cx: &RenderCtx<'_>) -> Value {
        self.pod.runtime.restart_policy = Some("Never".into());
        // The manifest travels as a reference only: an id and a location.
        self.pod.container.env.push(json!({
            "name": "RESTORE_MANIFEST_ID",
            "value": self.manifest.id.clone(),
        }));
        self.pod.container.env.push(json!({
            "name": "RESTORE_MANIFEST_LOCATION",
            "value": self.manifest.location.clone(),
        }));
        let component = self.pod.component.clone();
        let policy = self.policy;
        let mut metadata = cx.meta(&self.name, &component);
        metadata["finalizers"] = json!([RESTORE_FINALIZER]);
        json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": metadata,
            "spec": {
                "backoffLimit": policy.backoff_limit,
                "activeDeadlineSeconds": policy.active_deadline_seconds,
                "ttlSecondsAfterFinished": policy.ttl_seconds_after_finished,
                "template": self.pod.render(cx),
            },
        })
    }
}
