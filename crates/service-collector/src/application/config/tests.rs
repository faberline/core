use std::time::Duration;

use super::{ConfigError, RetryPolicy, RuntimeConfig};

fn retry() -> RetryPolicy {
    RetryPolicy::new(2, Duration::from_millis(1), Duration::from_millis(4)).unwrap()
}

#[test]
fn try_new_keeps_every_field() {
    let config = RuntimeConfig::try_new(10, 1024, retry(), true, Duration::from_millis(5)).unwrap();
    assert_eq!(config.batch_size(), 10);
    assert_eq!(config.max_record_bytes(), 1024);
    assert_eq!(config.retry(), retry());
    assert!(config.follow());
    assert_eq!(config.follow_poll_interval(), Duration::from_millis(5));
}

#[test]
fn try_new_rejects_zero_limits() {
    let poll = Duration::from_millis(1);
    assert_eq!(
        RuntimeConfig::try_new(0, 1024, retry(), false, poll),
        Err(ConfigError::ZeroBatchSize)
    );
    assert_eq!(
        RuntimeConfig::try_new(10, 0, retry(), false, poll),
        Err(ConfigError::ZeroRecordBytes)
    );
    assert_eq!(
        RuntimeConfig::try_new(10, 1024, retry(), false, Duration::ZERO),
        Err(ConfigError::ZeroFollowPoll)
    );
}
