//! One public helper that builds an app's scheduled backup CronJob. Core owns
//! the CronJob name, schedule, concurrency policy, identity wiring, labels and
//! owner reference; the app supplies only the backup command and arguments.
//! The app can also set the pull policy, env, volumes, resources, container
//! security context, node selector, tolerations and extra selector labels.

use super::{ContainerPlan, CronJobPlan, LabelSet, PodPlan, PodRuntimePolicy, WorkloadPlanError};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct BackupCronJobPlan {
    name: String,
    schedule: String,
    image: String,
    command: Vec<String>,
    args: Vec<String>,
    service_account_name: String,
    image_pull_policy: Option<String>,
    env: Vec<Value>,
    volumes: Vec<Value>,
    volume_mounts: Vec<Value>,
    resources: Option<Value>,
    container_security_context: Option<Value>,
    node_selector: Value,
    tolerations: Vec<Value>,
    selector_labels: LabelSet,
}

impl BackupCronJobPlan {
    /// A scheduled backup CronJob that runs `command` with `args` as `image`
    /// under the service account `service_account_name`.
    pub fn new(
        name: impl Into<String>,
        schedule: impl Into<String>,
        image: impl Into<String>,
        command: Vec<String>,
        args: Vec<String>,
        service_account_name: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            schedule: schedule.into(),
            image: image.into(),
            command,
            args,
            service_account_name: service_account_name.into(),
            image_pull_policy: None,
            env: Vec::new(),
            volumes: Vec::new(),
            volume_mounts: Vec::new(),
            resources: None,
            container_security_context: None,
            node_selector: serde_json::json!({}),
            tolerations: Vec::new(),
            selector_labels: LabelSet::new(),
        }
    }

    /// The CronJob name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the container image pull policy.
    pub fn with_image_pull_policy(mut self, policy: impl Into<String>) -> Self {
        self.image_pull_policy = Some(policy.into());
        self
    }

    /// Sets the container environment variables.
    pub fn with_env(mut self, env: Vec<Value>) -> Self {
        self.env = env;
        self
    }

    /// Sets the pod volumes.
    pub fn with_volumes(mut self, volumes: Vec<Value>) -> Self {
        self.volumes = volumes;
        self
    }

    /// Sets the container volume mounts.
    pub fn with_volume_mounts(mut self, mounts: Vec<Value>) -> Self {
        self.volume_mounts = mounts;
        self
    }

    /// Sets the container resource requests and limits.
    pub fn with_resources(mut self, resources: Value) -> Self {
        self.resources = Some(resources);
        self
    }

    /// Sets the container security context.
    pub fn with_container_security_context(mut self, context: Value) -> Self {
        self.container_security_context = Some(context);
        self
    }

    /// Sets the pod node selector.
    pub fn with_node_selector(mut self, selector: Value) -> Self {
        self.node_selector = selector;
        self
    }

    /// Sets the pod tolerations.
    pub fn with_tolerations(mut self, tolerations: Vec<Value>) -> Self {
        self.tolerations = tolerations;
        self
    }

    /// Adds extra pod and selector labels.
    pub fn with_selector_labels(mut self, labels: LabelSet) -> Self {
        self.selector_labels = labels;
        self
    }

    /// Builds the CronJob plan. Fails with [`WorkloadPlanError::EmptyCommand`]
    /// when `command` is empty.
    pub fn into_cron_job(self) -> Result<CronJobPlan, WorkloadPlanError> {
        if self.command.is_empty() {
            return Err(WorkloadPlanError::EmptyCommand(self.name));
        }
        let mut container = ContainerPlan::new("backup", self.image, self.args);
        container.command = self.command;
        container.image_pull_policy = self.image_pull_policy;
        container.env = self.env;
        container.volume_mounts = self.volume_mounts;
        container.security_context = self.container_security_context;
        container.resources = self.resources;
        let runtime = PodRuntimePolicy::restricted(self.service_account_name, self.node_selector)
            .with_tolerations(self.tolerations)
            .with_volumes(self.volumes);
        let pod =
            PodPlan::new("backup", container, runtime).with_selector_labels(self.selector_labels);
        Ok(CronJobPlan::new(self.name, self.schedule, pod))
    }
}
