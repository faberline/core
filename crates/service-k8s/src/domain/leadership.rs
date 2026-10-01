//! Leader election: the flag that says whether this replica may act, the rule
//! for taking the Lease that decides it, and the port that keeps it current.
//!
//! Every operator replica runs the watch + reconcile loop, but only the replica
//! that currently holds the `<manager>` Lease actually applies changes (the
//! reconcile loop gates on [`Election::is_leader`]). A [`LeaderLease`] acquires
//! and renews the Lease in the background; if the holder's renewal lapses past
//! the lease duration, another replica takes over ([`may_acquire`]).

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Shared leadership flag, flipped by the background election task and read by
/// the reconcile loop.
pub struct Election {
    pub is_leader: AtomicBool,
    pub identity: String,
}

impl Election {
    pub fn new(identity: String) -> Arc<Self> {
        Arc::new(Self {
            is_leader: AtomicBool::new(false),
            identity,
        })
    }
}

/// Keeps an [`Election`] current: once started, it acquires and renews the
/// leader Lease in the background and sets `is_leader` to match. Any failure
/// to reach the Lease means "not leader".
pub(crate) trait LeaderLease: Send + Sync {
    /// Start acquiring and renewing for `election`. Returns at once; the
    /// renewal runs until the process ends.
    fn start(&self, election: Arc<Election>);
}

/// Pure leadership decision: may `identity` hold the lease now? True when the
/// lease is unheld, already held by us, or expired (renewal lapsed past the
/// duration). False only when a *different* identity holds a still-fresh lease.
/// Factored out for unit testing — no cluster, no clock.
pub(crate) fn may_acquire(
    holder: Option<&str>,
    renew_epoch_secs: Option<i64>,
    lease_dur_secs: i64,
    identity: &str,
    now_epoch_secs: i64,
) -> bool {
    match holder {
        None => true,
        Some(h) if h == identity => true,
        Some(_) => match renew_epoch_secs {
            Some(r) => now_epoch_secs.saturating_sub(r) > lease_dur_secs,
            None => true,
        },
    }
}

#[cfg(test)]
mod tests;
