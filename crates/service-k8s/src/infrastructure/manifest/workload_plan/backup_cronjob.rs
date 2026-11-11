//! One public helper that builds an app's scheduled backup CronJob. Core owns
//! the CronJob name, schedule, concurrency policy, identity wiring, labels and
//! owner reference; the app supplies only the backup command and arguments.

use super::{ContainerPlan, CronJobPlan, PodPlan, PodRuntimePolicy, WorkloadPlanError};

#[derive(Clone, Debug)]
pub struct BackupCronJobPlan {
    name: String,
    schedule: String,
    image: String,
    command: Vec<String>,
    args: Vec<String>,
    service_account_name: String,
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
        }
    }

    /// The CronJob name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Builds the CronJob plan. Fails with [`WorkloadPlanError::EmptyCommand`]
    /// when `command` is empty.
    pub fn into_cron_job(self) -> Result<CronJobPlan, WorkloadPlanError> {
        if self.command.is_empty() {
            return Err(WorkloadPlanError::EmptyCommand(self.name));
        }
        let mut container = ContainerPlan::new("backup", self.image, self.args);
        container.command = self.command;
        let pod = PodPlan::new(
            "backup",
            container,
            PodRuntimePolicy::restricted(self.service_account_name, serde_json::json!({})),
        );
        Ok(CronJobPlan::new(self.name, self.schedule, pod))
    }
}
