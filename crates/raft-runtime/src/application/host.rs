//! `RaftHost` — drives a [`raft_core::RaftNode`] for a [`RaftStateMachine`] over
//! an h2c peer transport, with read-your-write `propose` and snapshot/compaction.
//!
//! Generalizes the per-service drivers (relay/lumen/keep each hand-rolled this):
//! the host is the **sole applier** — committed entries are fed to the state
//! machine in index order on a separate worker, so `propose` can return after the
//! command *applies* (not just commits), and `compact(applied, snapshot)` is
//! always sound.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use axum::http::StatusCode;
use raft_core::{
    DemotionRefused, Index, InstallSnapshotReq, InstallSnapshotResp, Membership, NodeId,
    PromotionRefused, RaftMsg, RaftNode, RemovalRefused, TransferRefused,
};
use serde::{Deserialize, Serialize};
use server_lifecycle::ShutdownDeadline;
use tokio::sync::{watch, Mutex, Notify, OwnedMutexGuard};
use tokio::task::{JoinHandle, JoinSet};

use crate::application::config::{HostConfig, SnapshotPolicy};
use crate::application::group::{GroupId, LEGACY_GROUP_ID};
use crate::application::port_error::StateMachineError;
use crate::application::state_machine::{
    AdmissionPermit, Command, RaftStateMachine, SnapshotPreparation,
};
use crate::infrastructure::{PeerTransport, RaftStore};

mod apply;
mod backpressure;
mod chunk_sink;
mod membership_ops;
mod ordered_apply;
mod outcome;
mod peer_lane;
mod propose;
mod shutdown;
mod shutdown_report;
mod snapshot_compaction;
mod spawn;
mod status;
mod voter_barriers;

pub use backpressure::ProposalBackpressure;
pub use chunk_sink::{ChunkSink, SNAPSHOT_CHUNK_SIZE};
pub use membership_ops::AdmissionRefused;
pub use outcome::{ProposalOutcome, SnapshotCompactionOutcome, StorageFailed};
pub use shutdown_report::{
    HostShutdownReport, LeadershipHandoff, PhaseRecord, PhaseStatus, ShutdownCaller, ShutdownPhase,
};
pub use status::{MembershipPhase, RaftStatus};

pub(crate) use apply::{apply_ready, cold_start, persist_node};
pub(crate) use backpressure::decode_backpressure;
pub(crate) use peer_lane::PeerLaneQueue;

use apply::tick_then_maybe_persist;
use backpressure::{encode_backpressure, rejected_admission};
use peer_lane::PeerLane;

pub(crate) struct Shared {
    pub(crate) id: NodeId,
    pub(crate) group_id: GroupId,
    pub(crate) node: Mutex<RaftNode>,
    pub(crate) store: RaftStore,
    pub(crate) sm: Arc<dyn RaftStateMachine>,
    /// Application permits become host-owned immediately after Raft assigned
    /// their index. They are keyed by index, never command content, so a caller
    /// cancellation cannot release a live appended command.
    pub(crate) pending_admission: StdMutex<BTreeMap<(Index, u64), AdmissionPermit>>,
    pub(crate) peers: StdRwLock<HashMap<NodeId, String>>,
    /// One coalescing RPC lane per peer. Raft's latest AppendEntries contains
    /// the complete missing suffix, so retaining every intermediate request
    /// only creates out-of-order progress and repeated durable writes.
    pub(crate) peer_lanes: StdRwLock<HashMap<NodeId, Arc<PeerLane>>>,
    pub(crate) client: reqwest::Client,
    pub(crate) peer_transport: Option<PeerTransport>,
    /// Fires (with the SM's applied head) whenever apply advances.
    pub(crate) applied_tx: watch::Sender<Index>,
    pub(crate) cfg: HostConfig,
    pub(crate) rpc_tracker: Arc<RpcTracker>,
    pub(crate) latched_failure: StdMutex<Option<StorageFailed>>,
    pub(crate) undeliverable_never_addressed: AtomicU64,
    pub(crate) undeliverable_withdrawn_address: AtomicU64,
    pub(crate) proposal_rejected_before_routing: AtomicU64,
    pub(crate) proposal_rejected_before_append: AtomicU64,
    pub(crate) lifecycle_generation: AtomicU64,
    pub(crate) snapshot_nonce: AtomicU64,
    pub(crate) snapshot_rpc_timeout: Duration,
    pub(crate) snapshot_install: Arc<Mutex<()>>,
    pub(crate) apply_running: AtomicBool,
    pub(crate) apply_stopped: AtomicBool,
    pub(crate) apply_tracker: Arc<RpcTracker>,
    pub(crate) max_resident_log_bytes: usize,
    pub(crate) shutdown_started: AtomicBool,
    pub(crate) shutdown_tx: watch::Sender<Option<HostShutdownReport>>,
}

#[derive(Default)]
pub(crate) struct RpcTracker {
    pub(crate) active: AtomicUsize,
    pub(crate) idle: Notify,
}

impl RpcTracker {
    pub(crate) async fn wait_idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::SeqCst) == 0 {
                return;
            }
            notified.await;
        }
    }
}

struct RpcGuard {
    tracker: Arc<RpcTracker>,
}

impl Drop for RpcGuard {
    fn drop(&mut self) {
        if self.tracker.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.tracker.idle.notify_waiters();
        }
    }
}

impl Shared {
    /// A public applied head requires both a completed callback and the current
    /// state machine's durable floor. A consumer reporting a lower floor must
    /// still be treated as lagging by all-voter safety barriers.
    pub(crate) fn completed_applied_index(&self) -> Index {
        (*self.applied_tx.borrow()).min(self.sm.applied_index())
    }

    /// Register direct peer work before checking the shutdown gate. The shared
    /// ordering with shutdown and wait_idle means an admitted operation cannot
    /// be missed by the drain. Proposal quiesce alone still allows final flush.
    fn begin_coordinated_peer_work(&self) -> Result<Arc<RpcGuard>> {
        let tracker = Arc::clone(&self.rpc_tracker);
        tracker.active.fetch_add(1, Ordering::SeqCst);
        let guard = Arc::new(RpcGuard { tracker });
        if self.shutdown_started.load(Ordering::SeqCst) {
            return Err(anyhow!(
                "raft: coordinated peer work is closed because shutdown has started"
            ));
        }
        Ok(guard)
    }

    pub(crate) fn http_client(&self) -> reqwest::Client {
        self.peer_transport
            .as_ref()
            .map(PeerTransport::http_client)
            .unwrap_or_else(|| self.client.clone())
    }

    pub(crate) fn persist(&self, node: &RaftNode) -> Result<(), StorageFailed> {
        if let Some(err) = self.latched_failure.lock().unwrap().clone() {
            return Err(err);
        }
        match persist_node(&self.store, node) {
            Ok(()) => Ok(()),
            Err(e) => {
                let err = StorageFailed {
                    node_id: self.id,
                    operation: "save",
                    path: self.store.path().to_path_buf(),
                    kind: e.kind(),
                };
                *self.latched_failure.lock().unwrap() = Some(err.clone());
                Err(err)
            }
        }
    }

    /// Schedule the sole ordered worker after the selected core state is durable.
    /// The caller holds node only for this metadata-only scheduling boundary.
    pub(crate) fn apply_ready(self: &Arc<Self>, node: &mut RaftNode) {
        self.schedule_apply(node);
    }

    pub(crate) fn leader_url(&self, node: &RaftNode) -> (Option<NodeId>, Option<String>) {
        let leader = node.leader();
        let url = leader.and_then(|l| {
            self.peers
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .get(&l)
                .cloned()
        });
        (leader, url)
    }
}

/// A running raft group host. Cheap to hold; aborts its tasks on drop.
pub struct RaftHost {
    pub(crate) shared: Arc<Shared>,
    tasks: StdMutex<Option<(JoinHandle<()>, JoinHandle<()>)>>,
}

impl Drop for RaftHost {
    fn drop(&mut self) {
        self.shared.apply_stopped.store(true, Ordering::Release);
        if let Some((tick, pump)) = self.tasks.lock().expect("raft task mutex poisoned").take() {
            tick.abort();
            pump.abort();
        }
    }
}

/// Pull the single reply addressed to `to` out of the node's outbox.
pub(crate) fn take_reply(node: &mut RaftNode, to: NodeId) -> Option<RaftMsg> {
    let mut reply = None;
    for o in node.take_outgoing() {
        if o.to == to
            && reply.is_none()
            && matches!(
                o.msg,
                RaftMsg::VoteResp(_) | RaftMsg::AppendResp(_) | RaftMsg::InstallSnapshotResp(_)
            )
        {
            reply = Some(o.msg);
        }
    }
    reply
}

#[cfg(test)]
mod tests;
