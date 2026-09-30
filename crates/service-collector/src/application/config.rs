use std::time::Duration;

use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl RetryPolicy {
    pub fn new(
        max_retries: usize,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Result<Self, ConfigError> {
        let policy = Self {
            max_retries,
            initial_backoff,
            max_backoff,
        };
        if initial_backoff.is_zero() {
            return Err(ConfigError::ZeroInitialBackoff);
        }
        if max_backoff < initial_backoff {
            return Err(ConfigError::InvalidMaxBackoff);
        }
        Ok(policy)
    }

    pub fn delay(&self, attempt: usize) -> Duration {
        let multiplier = 1_u32.checked_shl(attempt.min(6) as u32).unwrap_or(64);
        self.initial_backoff
            .saturating_mul(multiplier)
            .min(self.max_backoff)
    }
}

/// How one collector run reads and delivers. `try_new` is the only way to
/// build it, so every value has a positive batch size, record byte limit and
/// follow poll interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    batch_size: usize,
    max_record_bytes: usize,
    retry: RetryPolicy,
    follow: bool,
    follow_poll_interval: Duration,
}

impl RuntimeConfig {
    /// A config, or the `ConfigError` for a zero batch size, record byte
    /// limit or follow poll interval.
    pub fn try_new(
        batch_size: usize,
        max_record_bytes: usize,
        retry: RetryPolicy,
        follow: bool,
        follow_poll_interval: Duration,
    ) -> Result<Self, ConfigError> {
        if batch_size == 0 {
            return Err(ConfigError::ZeroBatchSize);
        }
        if max_record_bytes == 0 {
            return Err(ConfigError::ZeroRecordBytes);
        }
        if follow_poll_interval.is_zero() {
            return Err(ConfigError::ZeroFollowPoll);
        }
        Ok(Self {
            batch_size,
            max_record_bytes,
            retry,
            follow,
            follow_poll_interval,
        })
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn max_record_bytes(&self) -> usize {
        self.max_record_bytes
    }

    pub fn retry(&self) -> RetryPolicy {
        self.retry
    }

    pub fn follow(&self) -> bool {
        self.follow
    }

    pub fn follow_poll_interval(&self) -> Duration {
        self.follow_poll_interval
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ConfigError {
    #[error("collector batch size must be positive")]
    ZeroBatchSize,
    #[error("collector record byte limit must be positive")]
    ZeroRecordBytes,
    #[error("collector initial retry backoff must be positive")]
    ZeroInitialBackoff,
    #[error("collector maximum backoff must not be below its initial backoff")]
    InvalidMaxBackoff,
    #[error("collector follow poll interval must be positive")]
    ZeroFollowPoll,
}

/// Controls delivery only. Source polling still follows `RuntimeConfig::follow`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryRetryMode {
    /// Stop after `RetryPolicy::max_retries`, as the original API does.
    Bounded,
    /// Keep the same bounded batch on retryable failures, with capped backoff.
    /// Dropping the runtime future cancels delivery without committing its cursors.
    /// Permanent failures and invalid success receipts still stop the runtime.
    UntilCancelled,
}

#[cfg(test)]
mod tests;
