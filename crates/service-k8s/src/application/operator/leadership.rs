//! Whether this replica may act on a watched object: the leader-or-one-shot
//! rule the reconcile loop asks before it touches the cluster.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::leadership::{Election, LeaderLease};

/// How long a follower waits before it asks again. Short enough that a
/// replica which just took the Lease starts reconciling within seconds.
const FOLLOWER_REQUEUE: Duration = Duration::from_secs(10);

/// This replica's leadership, as the reconcile loop sees it.
///
/// There are two ways to get one, and both answer to the same gate:
/// - [`Leadership::campaign`] is the long-running operator's: a fresh
///   election, kept current by the Lease from then on.
/// - [`Leadership::held`] is a one-shot pass's: leadership the caller already
///   decided and hands over. It is taken as it is and never promoted.
///
/// Either way, a pass acts only while the election says "leader".
#[derive(Clone)]
pub(crate) struct Leadership {
    election: Arc<Election>,
}

impl Leadership {
    /// Campaign for the Lease: a fresh election for `identity` (not leader
    /// yet), which `lease` acquires and renews from now on.
    pub(crate) fn campaign(identity: String, lease: &dyn LeaderLease) -> Self {
        let election = Election::new(identity);
        lease.start(election.clone());
        Self { election }
    }

    /// The caller's own decision, for exactly one pass. Nothing here writes
    /// `is_leader`: a caller with no Lease cannot come out of this a leader.
    pub(crate) fn held(election: Arc<Election>) -> Self {
        Self { election }
    }

    /// This replica's identity, as the Lease holder and the Event reporter.
    pub(crate) fn identity(&self) -> &str {
        &self.election.identity
    }

    /// Whether this replica holds the Lease right now.
    pub(crate) fn is_leader(&self) -> bool {
        self.election.is_leader.load(Ordering::Relaxed)
    }

    /// `None` when this replica leads and may act; otherwise the requeue a
    /// follower answers with instead of reconciling.
    pub(crate) fn follower_requeue(&self) -> Option<Duration> {
        (!self.is_leader()).then_some(FOLLOWER_REQUEUE)
    }
}

#[cfg(test)]
mod tests;
