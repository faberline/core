//! The shard map: which shard owns which bucket, its ConfigMap form, and the
//! rebuild from pod reports. Pure; a pod reads the ConfigMap, this module never
//! writes it.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The key of the map inside the ConfigMap `data`.
pub const SHARD_MAP_KEY: &str = "shard-map.json";

/// Bucket index -> shard index, for a fixed bucket count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardMap {
    /// Rises by one on every change.
    pub version: u64,
    /// `buckets[b]` is the shard that owns bucket `b`.
    pub buckets: Vec<u32>,
}

/// What one pod says about the map it serves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodShardReport {
    /// The shard the pod serves.
    pub shard: u32,
    /// The buckets the pod owns.
    pub buckets: Vec<u32>,
    /// The version of the map the pod has loaded.
    pub map_version: u64,
}

/// Why a shard map could not be built or read.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ShardMapError {
    /// The rebuild got no pod report.
    #[error("no pod report")]
    NoReports,
    /// Two reports at the newest version name different owners for a bucket.
    #[error("reports of version {version} disagree about the owner of bucket {bucket}")]
    Conflict { version: u64, bucket: u32 },
    /// No report owns this bucket.
    #[error("no report owns bucket {0}")]
    MissingBucket(u32),
    /// A report names a bucket beyond the bucket count.
    #[error("bucket {bucket} is outside the {bucket_count} buckets of the map")]
    BucketOutOfRange { bucket: u32, bucket_count: u32 },
    /// The ConfigMap has no readable shard map.
    #[error("invalid shard-map ConfigMap: {0}")]
    InvalidConfigMap(String),
}

impl ShardMap {
    /// Round-robin: bucket `b` goes to shard `b % shard_count`, version 1.
    pub fn balanced(bucket_count: u32, shard_count: u32) -> ShardMap {
        let shards = shard_count.max(1);
        ShardMap {
            version: 1,
            buckets: (0..bucket_count).map(|b| b % shards).collect(),
        }
    }

    /// A copy with `buckets` moved to `to_shard` and the version raised by one.
    /// Buckets outside the map are ignored.
    pub fn with_moved_buckets(&self, buckets: &[u32], to_shard: u32) -> ShardMap {
        let mut moved = self.clone();
        moved.version = moved.version.saturating_add(1);
        for &b in buckets {
            if let Some(owner) = moved.buckets.get_mut(b as usize) {
                *owner = to_shard;
            }
        }
        moved
    }

    /// The ConfigMap that holds this map, with the map as a JSON string.
    pub fn to_configmap(&self, name: &str, namespace: &str) -> Value {
        let map = serde_json::to_string(self).unwrap();
        json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {"name": name, "namespace": namespace},
            "data": {SHARD_MAP_KEY: map},
        })
    }

    /// Reads the map back from a ConfigMap built by [`ShardMap::to_configmap`].
    pub fn from_configmap(configmap: &Value) -> Result<ShardMap, ShardMapError> {
        let raw = configmap["data"][SHARD_MAP_KEY]
            .as_str()
            .ok_or_else(|| ShardMapError::InvalidConfigMap(format!("no string {SHARD_MAP_KEY}")))?;
        serde_json::from_str::<ShardMap>(raw)
            .map_err(|e| ShardMapError::InvalidConfigMap(e.to_string()))
    }
}

/// Rebuilds the map from pod reports. Only the newest version counts; a bucket
/// must have exactly one owner among those reports.
pub fn rebuild_shard_map(
    reports: &[PodShardReport],
    bucket_count: u32,
) -> Result<ShardMap, ShardMapError> {
    let newest = reports
        .iter()
        .map(|r| r.map_version)
        .max()
        .ok_or(ShardMapError::NoReports)?;

    let mut owner: BTreeMap<u32, u32> = BTreeMap::new();
    for report in reports.iter().filter(|r| r.map_version == newest) {
        for &bucket in &report.buckets {
            if bucket >= bucket_count {
                return Err(ShardMapError::BucketOutOfRange {
                    bucket,
                    bucket_count,
                });
            }
            match owner.get(&bucket) {
                Some(&existing) if existing != report.shard => {
                    return Err(ShardMapError::Conflict {
                        version: newest,
                        bucket,
                    });
                }
                _ => {
                    owner.insert(bucket, report.shard);
                }
            }
        }
    }

    let mut buckets = Vec::with_capacity(bucket_count as usize);
    for b in 0..bucket_count {
        match owner.get(&b) {
            Some(&shard) => buckets.push(shard),
            None => return Err(ShardMapError::MissingBucket(b)),
        }
    }
    Ok(ShardMap {
        version: newest,
        buckets,
    })
}

#[cfg(test)]
mod tests;
