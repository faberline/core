use std::sync::atomic::{AtomicU64, Ordering};

/// A monotonic Prometheus counter: a single `AtomicU64` incremented with
/// `Ordering::Relaxed` (counters have no other state to stay consistent
/// with, so relaxed ordering is sufficient).
#[derive(Debug, Default)]
pub struct Counter(AtomicU64);

impl Counter {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Increment by 1.
    pub fn incr(&self) {
        self.add(1);
    }

    /// Increment by `n`.
    pub fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    /// Current value.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Deref to the underlying `AtomicU64` for callers that need raw
/// atomic ops (e.g. an observable-instrument callback holding only a
/// `&Counter`); `get`/`add`/`incr` above cover the common paths.
impl std::ops::Deref for Counter {
    type Target = AtomicU64;

    fn deref(&self) -> &AtomicU64 {
        &self.0
    }
}

/// A point-in-time Prometheus gauge: a single `AtomicU64` set with
/// `Ordering::Relaxed`.
#[derive(Debug, Default)]
pub struct Gauge(AtomicU64);

impl Gauge {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Overwrite the current value.
    pub fn set(&self, value: u64) {
        self.0.store(value, Ordering::Relaxed);
    }

    /// Current value.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Deref to the underlying `AtomicU64`, mirroring [`Counter`]'s escape
/// hatch for raw atomic ops.
impl std::ops::Deref for Gauge {
    type Target = AtomicU64;

    fn deref(&self) -> &AtomicU64 {
        &self.0
    }
}

/// A latency/duration observation: a `sum` + `count` counter pair, the
/// shape a Prometheus summary/histogram takes without bucket
/// boundaries. `observe` records one duration in whatever unit the
/// caller's metric name promises (lumen uses milliseconds).
#[derive(Debug, Default)]
pub struct Latency {
    sum: Counter,
    count: Counter,
}

impl Latency {
    pub const fn new() -> Self {
        Self {
            sum: Counter::new(),
            count: Counter::new(),
        }
    }

    /// Record one observation of `value`.
    pub fn observe(&self, value: u64) {
        self.sum.add(value);
        self.count.incr();
    }

    /// The running total of observed values, the `_sum` series.
    pub fn sum(&self) -> &Counter {
        &self.sum
    }

    /// The number of observations, the `_count` series.
    pub fn count(&self) -> &Counter {
        &self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_add_and_incr_accumulate() {
        let c = Counter::new();
        c.incr();
        c.add(4);
        assert_eq!(c.get(), 5);
    }

    #[test]
    fn counter_and_gauge_deref_to_raw_atomic() {
        let c = Counter::new();
        c.add(2);
        assert_eq!(c.load(Ordering::Relaxed), 2);

        let g = Gauge::new();
        g.set(9);
        assert_eq!(g.load(Ordering::Relaxed), 9);
    }

    #[test]
    fn gauge_set_overwrites() {
        let g = Gauge::new();
        g.set(10);
        g.set(3);
        assert_eq!(g.get(), 3);
    }

    #[test]
    fn latency_observe_tracks_sum_and_count() {
        let l = Latency::new();
        l.observe(7);
        l.observe(9);
        assert_eq!(l.sum().get(), 16);
        assert_eq!(l.count().get(), 2);
    }
}
