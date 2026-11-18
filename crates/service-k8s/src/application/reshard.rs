//! The reshard controller step: decide a split, run the step machine, resume after a failover.

use crate::domain::capacity::{ObservedShardUsage, ShardSplitPolicy};
use crate::domain::reshard::{
    decide_split, next_action, rollback_plan, BucketData, BucketDataError, CopyPass,
    ObservedReshard, ReshardAction, ReshardState, ReshardStep, ReshardWait, RollbackAction,
    SplitDecision, StepOutcome,
};
use crate::domain::shard_map::{rebuild_shard_map, PodShardReport, ShardMap, ShardMapError};

/// What the controller needs from the cluster. Synchronous; a fake implements it in tests.
pub trait ReshardCluster {
    /// The `ReshardState` in the CR status, `None` when no reshard is running.
    fn read_state(&mut self) -> Result<Option<ReshardState>, ReshardError>;
    /// Store `state` in the CR status.
    fn write_state(&mut self, state: &ReshardState) -> Result<(), ReshardError>;
    /// Remove the `ReshardState` from the CR status.
    fn clear_state(&mut self) -> Result<(), ReshardError>;
    /// The number of shards that the CR asks for.
    fn shard_count(&mut self) -> Result<u32, ReshardError>;
    /// Change the shard count of the CR. Only this controller calls it.
    fn set_shard_count(&mut self, shard_count: u32) -> Result<(), ReshardError>;
    /// The shard-map ConfigMap, `None` when it is lost.
    fn read_shard_map(&mut self) -> Result<Option<ShardMap>, ReshardError>;
    /// Create or replace the shard-map ConfigMap.
    fn write_shard_map(&mut self, map: &ShardMap) -> Result<(), ReshardError>;
    /// What each pod reports about its shard and its map version.
    fn pod_reports(&mut self) -> Result<Vec<PodShardReport>, ReshardError>;
    /// Create the StatefulSet and headless Service of `shard`.
    fn create_shard(&mut self, shard: u32) -> Result<(), ReshardError>;
    /// Delete the StatefulSet and headless Service of `shard`.
    fn delete_shard(&mut self, shard: u32) -> Result<(), ReshardError>;
}

/// The inputs that do not change between calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReshardConfig {
    /// The number of buckets of the shard map.
    pub bucket_count: u32,
    /// When a shard is split.
    pub policy: ShardSplitPolicy,
}

/// Why a reshard call failed.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ReshardError {
    /// The cluster refused a read or a write.
    #[error("cluster: {0}")]
    Cluster(String),
    /// A service's bucket hook failed.
    #[error(transparent)]
    Data(#[from] BucketDataError),
    /// The shard map could not be read or rebuilt.
    #[error(transparent)]
    Map(#[from] ShardMapError),
    /// The stored state and the cluster disagree.
    #[error("inconsistent reshard state: {0}")]
    Inconsistent(String),
}

/// How one call of `run_reshard` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReshardOutcome {
    /// No reshard was running and `decide_split` said `Hold` (or the shard owns fewer than 2 buckets).
    NoSplit,
    /// The split is finished; the map is at `map_version`.
    Completed {
        /// The map version after the split.
        map_version: u64,
    },
    /// A step failed; the rollback ran; the shard count is unchanged. `error` is the failure text.
    RolledBack {
        /// The step that failed.
        from_step: ReshardStep,
        /// The failure text.
        error: String,
    },
    /// The step machine waits (for example pods still on the old map version). Call again later.
    Waiting(ReshardWait),
}

/// The steps in the order in which they run.
const ORDER: [ReshardStep; 8] = [
    ReshardStep::Fence,
    ReshardStep::CopyFirst,
    ReshardStep::CopySecond,
    ReshardStep::Prune,
    ReshardStep::Checkpoint,
    ReshardStep::Cutover,
    ReshardStep::Converge,
    ReshardStep::Done,
];

fn position(step: ReshardStep) -> usize {
    ORDER.iter().position(|s| *s == step).unwrap()
}

fn observed(
    cluster: &mut dyn ReshardCluster,
    state: &ReshardState,
    outcome: StepOutcome,
) -> Result<ObservedReshard, ReshardError> {
    // Steps run in order, so every step before the current one has succeeded.
    let completed = ORDER[..position(state.step)].to_vec();
    let pod_map_versions = cluster
        .pod_reports()?
        .iter()
        .map(|pod| pod.map_version)
        .collect();
    Ok(ObservedReshard {
        outcome,
        completed,
        pod_map_versions,
    })
}

fn current_map(
    cluster: &mut dyn ReshardCluster,
    bucket_count: u32,
) -> Result<ShardMap, ReshardError> {
    if let Some(map) = cluster.read_shard_map()? {
        return Ok(map);
    }
    let map = rebuild_shard_map(&cluster.pod_reports()?, bucket_count)?;
    cluster.write_shard_map(&map)?;
    Ok(map)
}

fn execute(
    cluster: &mut dyn ReshardCluster,
    data: &dyn BucketData,
    state: &ReshardState,
    config: &ReshardConfig,
    step: ReshardStep,
) -> Result<(), ReshardError> {
    let (from, to) = (state.from_shard, state.to_shard);
    match step {
        ReshardStep::Fence => {
            for bucket in &state.buckets {
                data.fence(from, *bucket)?;
            }
        }
        ReshardStep::CopyFirst => {
            for bucket in &state.buckets {
                data.copy_pass(from, to, *bucket, CopyPass::First)?;
            }
        }
        ReshardStep::CopySecond => {
            for bucket in &state.buckets {
                data.copy_pass(from, to, *bucket, CopyPass::Second)?;
            }
        }
        ReshardStep::Prune => {
            for bucket in &state.buckets {
                data.prune(from, *bucket)?;
            }
        }
        ReshardStep::Checkpoint => {
            data.checkpoint(from)?;
            data.checkpoint(to)?;
        }
        ReshardStep::Cutover => {
            let map = current_map(cluster, config.bucket_count)?;
            let new = map.with_moved_buckets(&state.buckets, to);
            if new.version != state.map_version {
                return Err(ReshardError::Inconsistent(format!(
                    "map version {} but the move writes {}",
                    new.version, state.map_version
                )));
            }
            cluster.write_shard_map(&new)?;
            cluster.set_shard_count(to + 1)?;
        }
        ReshardStep::Converge | ReshardStep::Done => {}
    }
    Ok(())
}

fn roll_back(
    cluster: &mut dyn ReshardCluster,
    data: &dyn BucketData,
    state: &ReshardState,
    config: &ReshardConfig,
    from_step: ReshardStep,
) -> Result<(), ReshardError> {
    for action in rollback_plan(from_step) {
        match action {
            RollbackAction::Unfence => {
                for bucket in &state.buckets {
                    data.unfence(state.from_shard, *bucket)?;
                }
            }
            RollbackAction::DropTargetCopy => cluster.delete_shard(state.to_shard)?,
            RollbackAction::RestoreOldMapVersion => {
                let map = current_map(cluster, config.bucket_count)?;
                if map.version >= state.map_version {
                    cluster.write_shard_map(
                        &map.with_moved_buckets(&state.buckets, state.from_shard),
                    )?;
                    cluster.set_shard_count(state.to_shard)?;
                }
            }
        }
    }
    Ok(())
}

/// Run one call of the reshard controller. With no stored state it decides a split and
/// starts one; with a stored state it resumes at the stored step.
pub fn run_reshard(
    cluster: &mut dyn ReshardCluster,
    data: &dyn BucketData,
    config: ReshardConfig,
) -> Result<ReshardOutcome, ReshardError> {
    let stored = cluster.read_state()?;
    let mut state = match stored {
        Some(state) => state,
        None => {
            let count = cluster.shard_count()?;
            let mut usage = Vec::new();
            for shard_index in 0..count {
                usage.push(ObservedShardUsage {
                    shard_index,
                    durable_bytes: data.usage_bytes(shard_index)?,
                });
            }
            let (from_shard, new_shard_count) = match decide_split(count, config.policy, &usage) {
                SplitDecision::Hold => return Ok(ReshardOutcome::NoSplit),
                SplitDecision::Split {
                    from_shard,
                    new_shard_count,
                } => (from_shard, new_shard_count),
            };
            let to_shard = new_shard_count - 1;
            let map = current_map(cluster, config.bucket_count)?;
            let owned: Vec<u32> = (0..map.buckets.len() as u32)
                .filter(|bucket| map.buckets[*bucket as usize] == from_shard)
                .collect();
            if owned.len() < 2 {
                return Ok(ReshardOutcome::NoSplit);
            }
            let moved = owned[owned.len().div_ceil(2)..].to_vec();
            cluster.create_shard(to_shard)?;
            let state = ReshardState::new(from_shard, to_shard, moved, map.version + 1);
            cluster.write_state(&state)?;
            state
        }
    };

    loop {
        let action = next_action(&state, &observed(cluster, &state, StepOutcome::NotStarted)?);
        match action {
            ReshardAction::Finished => {
                cluster.clear_state()?;
                return Ok(ReshardOutcome::Completed {
                    map_version: state.map_version,
                });
            }
            ReshardAction::Wait(wait) => return Ok(ReshardOutcome::Waiting(wait)),
            ReshardAction::Rollback { .. } => {
                return Err(ReshardError::Inconsistent(
                    "unexpected rollback".to_string(),
                ));
            }
            ReshardAction::Run(step) => match execute(cluster, data, &state, &config, step) {
                Err(error) => {
                    roll_back(cluster, data, &state, &config, step)?;
                    cluster.clear_state()?;
                    return Ok(ReshardOutcome::RolledBack {
                        from_step: step,
                        error: error.to_string(),
                    });
                }
                Ok(()) if step == ReshardStep::Converge => {
                    let after =
                        next_action(&state, &observed(cluster, &state, StepOutcome::Succeeded)?);
                    match after {
                        ReshardAction::Finished => {
                            state.step = ReshardStep::Done;
                            cluster.clear_state()?;
                            return Ok(ReshardOutcome::Completed {
                                map_version: state.map_version,
                            });
                        }
                        ReshardAction::Wait(wait) => return Ok(ReshardOutcome::Waiting(wait)),
                        other => {
                            return Err(ReshardError::Inconsistent(format!(
                                "after Converge: {other:?}"
                            )))
                        }
                    }
                }
                Ok(()) => {
                    state.step = ORDER[position(step) + 1];
                    cluster.write_state(&state)?;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
