//! The reshard hook of a service and the storage-pressure decision. Pure:
//! nothing here talks to a cluster or a shard.
//! It also holds the pure step machine of a bucket move.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::capacity::{plan_shard_split, ObservedShardUsage, ShardSplitPolicy};

/// Which of the two copy passes of a bucket move this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyPass {
    First,
    Second,
}

/// What one copy pass moved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyReport {
    pub keys: u64,
    pub bytes: u64,
}

/// A failure reported by a service's [`BucketData`] hook.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum BucketDataError {
    #[error("shard {shard} is not reachable: {detail}")]
    Unreachable { shard: u32, detail: String },
    #[error("the service refused the step: {detail}")]
    Refused { detail: String },
}

/// The hook a service implements so that the core can move buckets between shards.
/// Sync and object-safe: the core holds it as `&dyn BucketData`.
pub trait BucketData {
    /// Durable bytes held by `shard`.
    fn usage_bytes(&self, shard: u32) -> Result<u64, BucketDataError>;
    /// Stop writes to `bucket` on `shard`.
    fn fence(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError>;
    /// Copy `bucket` from `from` to `to` in one pass.
    fn copy_pass(
        &self,
        from: u32,
        to: u32,
        bucket: u32,
        pass: CopyPass,
    ) -> Result<CopyReport, BucketDataError>;
    /// Remove `bucket` from `shard` after the move.
    fn prune(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError>;
    /// Make the state of `shard` durable before the membership changes.
    fn checkpoint(&self, shard: u32) -> Result<(), BucketDataError>;
    /// Resume writes to `bucket` on `shard`.
    fn unfence(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError>;
}

/// The answer of [`decide_split`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDecision {
    Hold,
    Split {
        from_shard: u32,
        new_shard_count: u32,
    },
}

/// Decide whether to split one shard, from the observed usage of each shard.
///
/// Wraps [`plan_shard_split`]. An invalid input holds: a bad input never splits.
pub fn decide_split(
    shard_count: u32,
    policy: ShardSplitPolicy,
    usage: &[ObservedShardUsage],
) -> SplitDecision {
    let Ok(plan) = plan_shard_split(shard_count, policy, usage) else {
        return SplitDecision::Hold;
    };
    match plan.busiest_shard {
        Some(busiest) if plan.desired_shard_count > plan.current_shard_count => {
            SplitDecision::Split {
                from_shard: busiest.shard_index,
                new_shard_count: plan.desired_shard_count,
            }
        }
        _ => SplitDecision::Hold,
    }
}

/// One step of a bucket move, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ReshardStep {
    /// Stop writes to the moving buckets on the source shard.
    Fence,
    /// The first copy pass of each moving bucket.
    CopyFirst,
    /// The second copy pass of each moving bucket.
    CopySecond,
    /// Remove the moved buckets from the source shard.
    Prune,
    /// Make the state of both shards durable.
    Checkpoint,
    /// Write the new shard map version.
    Cutover,
    /// Wait until every pod reports the new map version.
    Converge,
    /// The move is complete.
    Done,
}

/// The state of one bucket move. It is stored in the CR status, so a new
/// leader resumes the move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReshardState {
    /// The step the controller is on.
    pub step: ReshardStep,
    /// The shard that gives the buckets.
    pub from_shard: u32,
    /// The shard that takes the buckets.
    pub to_shard: u32,
    /// The virtual buckets that move.
    pub buckets: Vec<u32>,
    /// The new map version that `Cutover` writes.
    pub map_version: u64,
}

impl ReshardState {
    /// A new move. `step` is [`ReshardStep::Fence`].
    pub fn new(from_shard: u32, to_shard: u32, buckets: Vec<u32>, map_version: u64) -> Self {
        Self {
            step: ReshardStep::Fence,
            from_shard,
            to_shard,
            buckets,
            map_version,
        }
    }
}

/// What the controller saw for the step in [`ReshardState::step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step has not started.
    NotStarted,
    /// The step is running.
    Running,
    /// The step succeeded.
    Succeeded,
    /// The step failed.
    Failed,
}

/// What the controller observed for a bucket move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedReshard {
    /// The outcome of the step in [`ReshardState::step`].
    pub outcome: StepOutcome,
    /// Steps that were reported done (for example `CopyFirst`, `CopySecond`, `Checkpoint`).
    pub completed: Vec<ReshardStep>,
    /// The map version that each pod reports (one entry per pod).
    pub pod_map_versions: Vec<u64>,
}

/// Why [`next_action`] waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReshardWait {
    /// The step is still running.
    StepRunning(ReshardStep),
    /// A step that `Cutover` needs is not in `completed`.
    MissingStep(ReshardStep),
    /// Some pods still report the old map version.
    PodsOnOldMapVersion {
        /// The number of pods that report another version.
        stale: usize,
    },
    /// No pod has reported a map version yet.
    NoPodReports,
}

/// What the controller does next for a bucket move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReshardAction {
    /// Run this step.
    Run(ReshardStep),
    /// Wait before the next step.
    Wait(ReshardWait),
    /// Undo the move from this step (see [`rollback_plan`]).
    Rollback {
        /// The step that failed.
        from_step: ReshardStep,
    },
    /// The move is complete.
    Finished,
}

/// One undo action of a bucket move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackAction {
    /// Resume writes to the moving buckets on the source shard.
    Unfence,
    /// Drop the copied data on the target shard.
    DropTargetCopy,
    /// Restore the old shard map version.
    RestoreOldMapVersion,
}

/// The step after `step`, or `None` for `Done`.
fn following(step: ReshardStep) -> Option<ReshardStep> {
    use ReshardStep::*;
    match step {
        Fence => Some(CopyFirst),
        CopyFirst => Some(CopySecond),
        CopySecond => Some(Prune),
        Prune => Some(Checkpoint),
        Checkpoint => Some(Cutover),
        Cutover => Some(Converge),
        Converge => Some(Done),
        Done => None,
    }
}

/// Run `step`, unless it is `Cutover` and a copy pass or the checkpoint is
/// not reported done. `Cutover` is the only step that this guard covers.
fn start(step: ReshardStep, observed: &ObservedReshard) -> ReshardAction {
    if step == ReshardStep::Cutover {
        let needed = [
            ReshardStep::CopyFirst,
            ReshardStep::CopySecond,
            ReshardStep::Checkpoint,
        ];
        if let Some(missing) = needed.into_iter().find(|s| !observed.completed.contains(s)) {
            return ReshardAction::Wait(ReshardWait::MissingStep(missing));
        }
    }
    ReshardAction::Run(step)
}

/// What the controller does next for the move in `state`, given what it observed.
pub fn next_action(state: &ReshardState, observed: &ObservedReshard) -> ReshardAction {
    if state.step == ReshardStep::Done {
        return ReshardAction::Finished;
    }
    match observed.outcome {
        StepOutcome::Failed => ReshardAction::Rollback {
            from_step: state.step,
        },
        StepOutcome::Running => ReshardAction::Wait(ReshardWait::StepRunning(state.step)),
        StepOutcome::NotStarted => start(state.step, observed),
        StepOutcome::Succeeded if state.step == ReshardStep::Converge => {
            if observed.pod_map_versions.is_empty() {
                return ReshardAction::Wait(ReshardWait::NoPodReports);
            }
            let stale = observed
                .pod_map_versions
                .iter()
                .filter(|v| **v != state.map_version)
                .count();
            if stale > 0 {
                ReshardAction::Wait(ReshardWait::PodsOnOldMapVersion { stale })
            } else {
                ReshardAction::Finished
            }
        }
        StepOutcome::Succeeded => match following(state.step) {
            Some(next) => start(next, observed),
            None => ReshardAction::Finished,
        },
    }
}

/// The undo actions for a move that failed at `step`, in the order to run them.
pub fn rollback_plan(step: ReshardStep) -> Vec<RollbackAction> {
    use ReshardStep::*;
    match step {
        Fence | CopyFirst | CopySecond | Prune | Checkpoint => {
            vec![RollbackAction::DropTargetCopy, RollbackAction::Unfence]
        }
        Cutover | Converge => vec![
            RollbackAction::RestoreOldMapVersion,
            RollbackAction::Unfence,
        ],
        Done => vec![],
    }
}

#[cfg(test)]
mod tests;
