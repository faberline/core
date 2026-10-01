use super::*;

/// Return the absolute cumulative cutoff for each sequential shutdown phase.
///
/// The final cutoff is the caller's original usable end. Any duration that
/// does not divide evenly into quarters remains available to the final phase.
pub(super) fn shutdown_phase_cutoffs(
    deadline: ShutdownDeadline,
    started_at: tokio::time::Instant,
) -> [tokio::time::Instant; 4] {
    let usable_end = deadline.expires_at() - deadline.reserve();
    let usable_interval = usable_end.saturating_duration_since(started_at);
    let quarter = usable_interval / 4;

    [
        started_at + quarter,
        started_at + quarter * 2,
        started_at + quarter * 3,
        usable_end,
    ]
}

/// Return whether a shutdown phase has reached its cumulative cutoff.
pub(super) fn shutdown_phase_cutoff_elapsed(cutoff: tokio::time::Instant) -> bool {
    tokio::time::Instant::now() >= cutoff
}

impl RaftHost {
    /// Quiesce proposal admission on this host. Returns `true` if this call
    /// transitioned admission from open to closed, or `false` if already closed.
    pub fn quiesce_proposals(&self) -> bool {
        self.shared
            .lifecycle_generation
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Run host shutdown bounded by the caller's shared absolute deadline (#3672, #3683).
    ///
    /// Executes the four shutdown phases in fixed sequential order:
    /// 1. `Quiesce` — stop accepting new proposals
    /// 2. `LeadershipHandoff` — hand off leadership to a caught-up peer
    /// 3. `BackgroundTasks` — abort and await the tick and pump loops
    /// 4. `PeerRpcDrain` — wait for in-flight peer RPCs to finish
    ///
    /// Every phase uses one cumulative absolute cutoff from the usable interval
    /// captured when shutdown begins. If a cutoff expires, the run stops
    /// immediately, later phases are neither run nor recorded, and
    /// `incomplete_phase` names that phase.
    ///
    /// A host performs this shutdown sequence at most once across its whole lifetime.
    /// The first caller executes the phases and receives a report with
    /// [`ShutdownCaller::Executed`]; concurrent or repeat callers wait for completion
    /// and receive the identical terminal outcome with [`ShutdownCaller::Joined`].
    pub async fn shutdown_within(&self, deadline: ShutdownDeadline) -> HostShutdownReport {
        if self
            .shared
            .shutdown_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            let mut rx = self.shared.shutdown_tx.subscribe();
            while rx.borrow().is_none() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
            let terminal = rx
                .borrow()
                .as_ref()
                .expect("shutdown report published")
                .clone();
            return HostShutdownReport {
                caller: ShutdownCaller::Joined,
                phases: terminal.phases,
                handoff: terminal.handoff,
                incomplete_phase: terminal.incomplete_phase,
                peer_listener_close_safe: terminal.peer_listener_close_safe,
                storage_failure: terminal.storage_failure,
            };
        }

        let report = self.execute_shutdown(deadline).await;
        self.shared.shutdown_tx.send_replace(Some(report.clone()));
        report
    }

    async fn execute_shutdown(&self, deadline: ShutdownDeadline) -> HostShutdownReport {
        let mut phases = Vec::with_capacity(4);
        let mut handoff = LeadershipHandoff::NotLeader;
        let mut observed_storage_failure = None;
        let phase_cutoffs = shutdown_phase_cutoffs(deadline, tokio::time::Instant::now());

        // Phase 1: Quiesce
        let start = Instant::now();
        if shutdown_phase_cutoff_elapsed(phase_cutoffs[0]) {
            phases.push(PhaseRecord {
                phase: ShutdownPhase::Quiesce,
                status: PhaseStatus::DeadlineExpired,
                elapsed: start.elapsed(),
            });
            return HostShutdownReport {
                caller: ShutdownCaller::Executed,
                phases,
                handoff,
                incomplete_phase: Some(ShutdownPhase::Quiesce),
                peer_listener_close_safe: false,
                storage_failure: self.shared.latched_failure.lock().unwrap().clone(),
            };
        }
        self.quiesce_proposals();
        let current_failure = self.shared.latched_failure.lock().unwrap().clone();
        let status = if observed_storage_failure.is_none() && current_failure.is_some() {
            observed_storage_failure = current_failure;
            PhaseStatus::StorageFailed
        } else {
            PhaseStatus::Completed
        };
        phases.push(PhaseRecord {
            phase: ShutdownPhase::Quiesce,
            status,
            elapsed: start.elapsed(),
        });

        // Phase 2: LeadershipHandoff
        let start = Instant::now();
        if shutdown_phase_cutoff_elapsed(phase_cutoffs[1]) {
            phases.push(PhaseRecord {
                phase: ShutdownPhase::LeadershipHandoff,
                status: PhaseStatus::DeadlineExpired,
                elapsed: start.elapsed(),
            });
            return HostShutdownReport {
                caller: ShutdownCaller::Executed,
                phases,
                handoff,
                incomplete_phase: Some(ShutdownPhase::LeadershipHandoff),
                peer_listener_close_safe: false,
                storage_failure: observed_storage_failure
                    .or_else(|| self.shared.latched_failure.lock().unwrap().clone()),
            };
        }
        match tokio::time::timeout_at(phase_cutoffs[1], self.handoff_leadership()).await {
            Ok(outcome) => {
                handoff = outcome;
                let current_failure = self.shared.latched_failure.lock().unwrap().clone();
                let status = if observed_storage_failure.is_none() && current_failure.is_some() {
                    observed_storage_failure = current_failure;
                    PhaseStatus::StorageFailed
                } else {
                    PhaseStatus::Completed
                };
                phases.push(PhaseRecord {
                    phase: ShutdownPhase::LeadershipHandoff,
                    status,
                    elapsed: start.elapsed(),
                });
            }
            Err(_) => {
                phases.push(PhaseRecord {
                    phase: ShutdownPhase::LeadershipHandoff,
                    status: PhaseStatus::DeadlineExpired,
                    elapsed: start.elapsed(),
                });
                return HostShutdownReport {
                    caller: ShutdownCaller::Executed,
                    phases,
                    handoff,
                    incomplete_phase: Some(ShutdownPhase::LeadershipHandoff),
                    peer_listener_close_safe: false,
                    storage_failure: observed_storage_failure
                        .or_else(|| self.shared.latched_failure.lock().unwrap().clone()),
                };
            }
        }

        // Phase 3: BackgroundTasks
        let start = Instant::now();
        let timed_out = if shutdown_phase_cutoff_elapsed(phase_cutoffs[2]) {
            true
        } else {
            let tasks = self.tasks.lock().expect("raft task mutex poisoned").take();
            if let Some((tick, pump)) = tasks {
                tick.abort();
                pump.abort();
                let shared = Arc::clone(&self.shared);
                let join_all = async move {
                    let _ = tick.await;
                    let _ = pump.await;
                    {
                        let _node = shared.node.lock().await;
                        shared.apply_stopped.store(true, Ordering::Release);
                    }
                    shared.apply_tracker.wait_idle().await;
                };
                tokio::time::timeout_at(phase_cutoffs[2], join_all)
                    .await
                    .is_err()
            } else {
                false
            }
        };
        if timed_out {
            phases.push(PhaseRecord {
                phase: ShutdownPhase::BackgroundTasks,
                status: PhaseStatus::DeadlineExpired,
                elapsed: start.elapsed(),
            });
            return HostShutdownReport {
                caller: ShutdownCaller::Executed,
                phases,
                handoff,
                incomplete_phase: Some(ShutdownPhase::BackgroundTasks),
                peer_listener_close_safe: false,
                storage_failure: observed_storage_failure
                    .or_else(|| self.shared.latched_failure.lock().unwrap().clone()),
            };
        }
        let current_failure = self.shared.latched_failure.lock().unwrap().clone();
        let status = if observed_storage_failure.is_none() && current_failure.is_some() {
            observed_storage_failure = current_failure;
            PhaseStatus::StorageFailed
        } else {
            PhaseStatus::Completed
        };
        phases.push(PhaseRecord {
            phase: ShutdownPhase::BackgroundTasks,
            status,
            elapsed: start.elapsed(),
        });

        // Phase 4: PeerRpcDrain
        let start = Instant::now();
        if shutdown_phase_cutoff_elapsed(phase_cutoffs[3])
            || tokio::time::timeout_at(phase_cutoffs[3], self.shared.rpc_tracker.wait_idle())
                .await
                .is_err()
        {
            phases.push(PhaseRecord {
                phase: ShutdownPhase::PeerRpcDrain,
                status: PhaseStatus::DeadlineExpired,
                elapsed: start.elapsed(),
            });
            return HostShutdownReport {
                caller: ShutdownCaller::Executed,
                phases,
                handoff,
                incomplete_phase: Some(ShutdownPhase::PeerRpcDrain),
                peer_listener_close_safe: false,
                storage_failure: observed_storage_failure
                    .or_else(|| self.shared.latched_failure.lock().unwrap().clone()),
            };
        }
        let current_failure = self.shared.latched_failure.lock().unwrap().clone();
        let status = if observed_storage_failure.is_none() && current_failure.is_some() {
            observed_storage_failure = current_failure;
            PhaseStatus::StorageFailed
        } else {
            PhaseStatus::Completed
        };
        phases.push(PhaseRecord {
            phase: ShutdownPhase::PeerRpcDrain,
            status,
            elapsed: start.elapsed(),
        });

        HostShutdownReport {
            caller: ShutdownCaller::Executed,
            phases,
            handoff,
            incomplete_phase: None,
            peer_listener_close_safe: true,
            storage_failure: observed_storage_failure,
        }
    }

    /// Stop the periodic host loops and wait for every already-dispatched peer
    /// RPC to finish before the h2 client is dropped. Service shutdown should
    /// stop public ingress first, call this method, then close the peer
    /// listener only when `peer_listener_close_safe` is true. The bounded wait
    /// prevents active HTTP/2 streams from being torn down with the Tokio
    /// runtime.
    pub async fn shutdown(&self) -> Result<()> {
        let timeout = self.shared.cfg.rpc_timeout() + self.shared.cfg.rpc_timeout();
        let deadline =
            ShutdownDeadline::from_now(timeout, Duration::ZERO).map_err(|e| anyhow!("{e}"))?;
        let report = self.shutdown_within(deadline).await;
        report.into_result()
    }
}
