//! Runs one membership step through the [`MembershipExecutor`] port.
//!
//! The planner is pure and the observed state is the only state, so a new
//! leader that observes again continues with the right step. Nothing is
//! remembered between calls.

use std::sync::atomic::Ordering;

use crate::domain::leadership::Election;
use crate::domain::membership::{
    plan_membership_step, DesiredMembership, MembershipStep, ObservedMembership,
};
use crate::domain::membership_port::{MembershipError, MembershipExecutor, Progress, StepResult};

/// Plans the next step from `observed` and `desired`, runs it through
/// `executor`, and reports what happened.
pub async fn run_membership_step<E: MembershipExecutor>(
    observed: &ObservedMembership,
    desired: &DesiredMembership,
    executor: &E,
) -> Result<Progress, MembershipError> {
    let result = match plan_membership_step(observed, desired) {
        MembershipStep::Done => return Ok(Progress::Done),
        MembershipStep::AddLearner { shard, replica } => {
            executor.add_learner(shard, replica).await?
        }
        MembershipStep::Promote { shard, replica } => executor.promote(shard, replica).await?,
        MembershipStep::TransferLeadership { shard, from, to } => {
            executor.transfer_leadership(shard, from, to).await?
        }
        MembershipStep::Demote { shard, replica } => executor.demote(shard, replica).await?,
        MembershipStep::RemoveMember { shard, replica } => {
            executor.remove_member(shard, replica).await?
        }
        MembershipStep::ScalePods { shard, replicas } => {
            executor.scale_pods(shard, replicas).await?
        }
        MembershipStep::DeletePvc { shard, replica } => executor.delete_pvc(shard, replica).await?,
    };
    Ok(match result {
        StepResult::Applied => Progress::Advanced,
        StepResult::Waiting(reason) => Progress::Waiting(reason),
    })
}

/// Like [`run_membership_step`], but only while this replica holds the leader
/// Lease. A follower runs nothing.
pub async fn run_membership_step_as_leader<E: MembershipExecutor>(
    election: &Election,
    observed: &ObservedMembership,
    desired: &DesiredMembership,
    executor: &E,
) -> Result<Progress, MembershipError> {
    if !election.is_leader.load(Ordering::Relaxed) {
        return Ok(Progress::Waiting("not the leader".to_string()));
    }
    run_membership_step(observed, desired, executor).await
}
