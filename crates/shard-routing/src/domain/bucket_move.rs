//! The buckets that change owner between two shard-map versions.

use anyhow::{bail, Result};

use crate::domain::virtual_bucket_shard_map::VirtualBucketShardMap;

/// One virtual bucket that moves from one physical shard to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BucketMove {
    pub bucket: u32,
    pub from_shard: u32,
    pub to_shard: u32,
}

/// The buckets whose owner differs between `old` and `new`, sorted by bucket, each bucket once.
/// Two maps with the same assignments give an empty list, even when their versions differ.
/// Both maps must have the same virtual bucket count.
pub fn bucket_moves(
    old: &VirtualBucketShardMap,
    new: &VirtualBucketShardMap,
) -> Result<Vec<BucketMove>> {
    if old.virtual_bucket_count() != new.virtual_bucket_count() {
        bail!(
            "shard maps have different virtual bucket counts: {} and {}",
            old.virtual_bucket_count(),
            new.virtual_bucket_count()
        );
    }
    let mut moves = Vec::new();
    for bucket in 0..old.virtual_bucket_count() {
        let from_shard = old
            .assignment_for_bucket(bucket)
            .expect("bucket is inside the map");
        let to_shard = new
            .assignment_for_bucket(bucket)
            .expect("bucket is inside the map");
        if from_shard != to_shard {
            moves.push(BucketMove {
                bucket,
                from_shard,
                to_shard,
            });
        }
    }
    Ok(moves)
}

#[cfg(test)]
mod tests;
