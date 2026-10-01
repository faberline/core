use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::google::Clock;

/// The production clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default()
    }
}
