//! Generic bounded asynchronous execution for service-owned work items.
//!
//! This crate owns concurrency mechanics only. Durable ownership, fencing,
//! retries, target semantics, and outcome persistence remain in each domain's
//! committed state machine.

mod bounded;
mod group_commit;
mod job_runner;

pub use bounded::run_bounded;
pub use group_commit::{
    spawn_group_commit, GroupCommitConfig, GroupCommitConfigError, GroupCommitError,
    GroupCommitQueue, GroupCommitRequest, GroupCommitWorker,
};
pub use job_runner::{JobRunReport, JobRunState, JobRunner, JobState};
