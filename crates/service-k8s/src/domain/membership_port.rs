//! Ports for running one membership step. The planner (`membership.rs`) is
//! pure; these traits are the effects it asks for.

use futures::future::BoxFuture;

/// What one executed step answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepResult {
    /// The step is done (a Raft change is committed).
    Applied,
    /// The step cannot be done yet, for example "learner not caught up".
    /// Nothing changed.
    Waiting(String),
}

/// What one `run_membership_step` call did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// A step ran and was applied.
    Advanced,
    /// A step could not run yet; the reason is given.
    Waiting(String),
    /// The observed state already matches the desired layout.
    Done,
}

/// Why a membership step failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipError {
    /// What failed, with the names involved.
    pub message: String,
}

impl MembershipError {
    /// Builds an error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for MembershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MembershipError {}

/// One method per `MembershipStep` variant except `Done`. A Raft method
/// returns only after the change is committed, so the next pod call never
/// overtakes it.
pub trait MembershipExecutor: Send + Sync {
    /// Sets the StatefulSet of `shard` to `replicas` pods.
    fn scale_pods<'a>(
        &'a self,
        shard: u32,
        replicas: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Adds `replica` to the shard as a learner.
    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Promotes the learner `replica` to a voter.
    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Moves leadership of `shard` from `from` to `to`.
    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Demotes the voter `replica` to a learner.
    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Removes the member `replica` from the shard.
    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Deletes the PVC of `replica`. Deleting a PVC that is already gone is
    /// done, so the step can run again.
    fn delete_pvc<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
}

/// The five Raft membership calls of a shard's leader. The HTTP implementation
/// is a follow-up; this crate only defines the port.
pub trait RaftAdminClient: Send + Sync {
    /// Adds `replica` to the shard as a learner and waits for the commit.
    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Promotes the learner `replica` to a voter and waits for the commit.
    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Moves leadership of `shard` from `from` to `to`.
    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Demotes the voter `replica` to a learner.
    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
    /// Removes the member `replica` from the shard.
    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>>;
}
