#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionRuntimeConfig {
    pub batch_size: usize,
    pub snapshot_interval_events: u64,
    pub retry_after_seconds: u64,
}

impl ProjectionRuntimeConfig {
    pub fn new(batch_size: usize, snapshot_interval_events: u64, retry_after_seconds: u64) -> Self {
        Self {
            batch_size: batch_size.max(1),
            snapshot_interval_events: snapshot_interval_events.max(1),
            retry_after_seconds: retry_after_seconds.max(1),
        }
    }
}
