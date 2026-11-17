//! One-hop forwarding between shards: header names, the errors of a forward, and the guards.

use std::fmt;

use anyhow::{anyhow, Result};
use http::{HeaderMap, HeaderValue};

use crate::domain::virtual_bucket_shard_map::VirtualBucketShardMap;

/// Neutral header that marks a request already forwarded once.
pub const FORWARDED_HEADER: &str = "x-faberline-forwarded";
/// Neutral header that carries the sender's shard-map version.
pub const MAP_VERSION_HEADER: &str = "x-faberline-map-version";
/// Legacy name of [`FORWARDED_HEADER`]; accepted on receipt, never written.
pub const LEGACY_FORWARDED_HEADER: &str = "x-lumen-forwarded";
/// Legacy name of [`MAP_VERSION_HEADER`]; accepted on receipt, never written.
pub const LEGACY_MAP_VERSION_HEADER: &str = "x-lumen-map-version";

/// The owner shard could not be reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardForwardUnavailable {
    pub shard: u32,
    pub detail: String,
}

/// The owner shard answered with an error status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardForwardRemoteError {
    pub shard: u32,
    pub status: u16,
    pub body: String,
}

/// The forwarded map version differs from the local one (a stale map).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardMapVersionMismatch {
    pub local: u64,
    pub forwarded: u64,
}

/// A forwarded request reached a shard that does not own its bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardForwardMisrouted {
    pub bucket: u32,
    pub local_shard: u32,
    pub owner_shard: u32,
}

impl fmt::Display for ShardForwardUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shard {} is unavailable: {}", self.shard, self.detail)
    }
}

impl std::error::Error for ShardForwardUnavailable {}

impl fmt::Display for ShardForwardRemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shard {} answered {}: {}",
            self.shard, self.status, self.body
        )
    }
}

impl std::error::Error for ShardForwardRemoteError {}

impl fmt::Display for ShardMapVersionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shard map version mismatch: local {}, forwarded {}",
            self.local, self.forwarded
        )
    }
}

impl std::error::Error for ShardMapVersionMismatch {}

impl fmt::Display for ShardForwardMisrouted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "forwarded request for bucket {} belongs to shard {}, not shard {}",
            self.bucket, self.owner_shard, self.local_shard
        )
    }
}

impl std::error::Error for ShardForwardMisrouted {}

/// True when the request carries the forwarded marker (neutral or legacy name) with value `1`.
pub fn is_forwarded(headers: &HeaderMap) -> bool {
    [FORWARDED_HEADER, LEGACY_FORWARDED_HEADER]
        .iter()
        .any(|name| headers.get(*name).is_some_and(|v| v.as_bytes() == b"1"))
}

/// The headers of a forwarded request: the marker and the sender's map version.
pub fn forward_headers(map_version: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(FORWARDED_HEADER, HeaderValue::from_static("1"));
    headers.insert(
        MAP_VERSION_HEADER,
        HeaderValue::from_str(&map_version.to_string()).expect("digits are a valid header value"),
    );
    headers
}

/// The map version of the request: the neutral name, else the legacy name, else `None`.
pub fn forwarded_map_version(headers: &HeaderMap) -> Result<Option<u64>> {
    let (name, value) = match headers.get(MAP_VERSION_HEADER) {
        Some(value) => (MAP_VERSION_HEADER, value),
        None => match headers.get(LEGACY_MAP_VERSION_HEADER) {
            Some(value) => (LEGACY_MAP_VERSION_HEADER, value),
            None => return Ok(None),
        },
    };
    let text = value.to_str().map_err(|_| {
        anyhow!(
            "header {name} is not a map version: {}",
            String::from_utf8_lossy(value.as_bytes())
        )
    })?;
    text.parse::<u64>().map(Some).map_err(|_| {
        anyhow!(
            "header {name} is not a map version: {}",
            String::from_utf8_lossy(value.as_bytes())
        )
    })
}

/// Refuses a forwarded request whose map version differs from the local map. A request without the header passes.
pub fn check_forwarded_map_version(headers: &HeaderMap, map: &VirtualBucketShardMap) -> Result<()> {
    match forwarded_map_version(headers)? {
        None => Ok(()),
        Some(forwarded) if forwarded == map.version() => Ok(()),
        Some(forwarded) => Err(anyhow::Error::new(ShardMapVersionMismatch {
            local: map.version(),
            forwarded,
        })),
    }
}

/// Refuses a forwarded request for a bucket that the local shard does not own. A request that was not forwarded passes.
pub fn assert_owns(
    headers: &HeaderMap,
    map: &VirtualBucketShardMap,
    local_shard: u32,
    collection_id: &str,
    routing_key: &str,
) -> Result<()> {
    if !is_forwarded(headers) {
        return Ok(());
    }
    let route = map.route_key(collection_id, routing_key);
    if route.shard == local_shard {
        return Ok(());
    }
    Err(anyhow::Error::new(ShardForwardMisrouted {
        bucket: route.bucket,
        local_shard,
        owner_shard: route.shard,
    }))
}

#[cfg(test)]
mod tests;
