//! A write fence: a bucket that is being moved refuses writes until the fence is cleared
//! or its time limit passes. The time limit is a backstop so a lost `clear` cannot block
//! a bucket for ever.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The longest time limit of one fence: 3600 seconds. A longer `ttl` is cut to this.
pub const MAX_FENCE_TTL: Duration = Duration::from_secs(3600);

/// The set of fenced buckets, each with the time at which its fence ends.
/// The caller passes `now`, so the type reads no clock.
#[derive(Debug, Default)]
pub struct WriteFence {
    armed: Mutex<HashMap<u32, Instant>>,
}

impl WriteFence {
    /// A fence set with no bucket fenced.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fence `bucket` from `now` for `ttl` (cut to [`MAX_FENCE_TTL`]). Arming a bucket
    /// that is already fenced replaces its end time.
    pub fn arm(&self, bucket: u32, ttl: Duration, now: Instant) {
        let ends = now + ttl.min(MAX_FENCE_TTL);
        self.lock().insert(bucket, ends);
    }

    /// Remove the fence of `bucket`. Returns `true` when a fence was there.
    pub fn clear(&self, bucket: u32) -> bool {
        self.lock().remove(&bucket).is_some()
    }

    /// `true` when `bucket` is fenced at `now`. A fence ends at its end time: at `now`
    /// equal to the end time the bucket is no longer blocked.
    pub fn blocks(&self, bucket: u32, now: Instant) -> bool {
        self.lock().get(&bucket).is_some_and(|ends| now < *ends)
    }

    /// `true` when at least one of `buckets` is fenced at `now`.
    pub fn blocks_any(&self, buckets: &[u32], now: Instant) -> bool {
        let armed = self.lock();
        buckets
            .iter()
            .any(|bucket| armed.get(bucket).is_some_and(|ends| now < *ends))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Instant>> {
        self.armed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
