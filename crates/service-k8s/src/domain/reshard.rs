//! The reshard hook of a service and the storage-pressure decision. Pure:
//! nothing here talks to a cluster or a shard.

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

#[cfg(test)]
mod tests;
