use super::*;
use crate::domain::{ForwardReply, PUBLISH_PATH};
use raft_core::Term;

/// The waiter owns this registration, including across cancellation. The applier
/// records the actual term, even when a different entry occupies the same index.
struct ProposalIdentity<'a> {
    shared: &'a Shared,
    index: Index,
    term: Term,
}

impl ProposalIdentity<'_> {
    fn committed_or_resident_term(&self, node: &RaftNode) -> Option<Term> {
        self.shared
            .proposal_committed_terms
            .lock()
            .unwrap()
            .get(&(self.index, self.term.get()))
            .copied()
            .flatten()
            .or_else(|| {
                // Only the boundary itself has an exact term after compaction.
                (self.index >= node.snapshot_index())
                    .then(|| node.term_at_index(self.index))
                    .flatten()
            })
    }
}

impl Drop for ProposalIdentity<'_> {
    fn drop(&mut self) {
        self.shared
            .proposal_committed_terms
            .lock()
            .unwrap()
            .remove(&(self.index, self.term.get()));
    }
}

impl Shared {
    /// Try a leader-side proposal and return its index once the **state machine
    /// applies** it (read-your-write). `None` means this node was no longer
    /// leader when the node lock was acquired, so the caller may safely
    /// re-route the command: it was not appended. Once an index is allocated,
    /// an apply timeout remains an error and must not be retried blindly.
    pub(crate) async fn try_propose_applied(
        self: &Arc<Self>,
        command: Command,
    ) -> Option<ProposalOutcome> {
        if self.lifecycle_generation.load(Ordering::Acquire) > 0 {
            self.proposal_rejected_before_append
                .fetch_add(1, Ordering::Relaxed);
            return Some(ProposalOutcome::RejectedBeforeAdmission {
                reason: "raft: proposal admission closed".to_string(),
            });
        }
        let permit = match self.sm.admit_proposal(&command) {
            Ok(permit) => permit,
            Err(error) => {
                self.proposal_rejected_before_append
                    .fetch_add(1, Ordering::Relaxed);
                return Some(rejected_admission(error));
            }
        };
        let (index, term, identity) = {
            let mut n = self.node.lock().await;
            if self.lifecycle_generation.load(Ordering::Acquire) > 0 {
                self.proposal_rejected_before_append
                    .fetch_add(1, Ordering::Relaxed);
                return Some(ProposalOutcome::RejectedBeforeAdmission {
                    reason: "raft: proposal admission closed".to_string(),
                });
            }
            if n.resident_log_bytes().saturating_add(command.len()) > self.max_resident_log_bytes {
                return Some(ProposalOutcome::RejectedBeforeAdmission {
                    reason: format!(
                        "raft: resident log memory limit reached ({} bytes); retry after snapshot compaction",
                        self.max_resident_log_bytes
                    ),
                });
            }
            let idx = n.propose(command)?;
            let term = n.current_term();
            self.proposal_committed_terms
                .lock()
                .unwrap()
                .insert((idx, term.get()), None);
            let identity = ProposalIdentity {
                shared: self,
                index: idx,
                term,
            };
            if let Some(permit) = permit {
                let previous = self
                    .pending_admission
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert((idx, term.get()), permit);
                debug_assert!(previous.is_none(), "Raft index permit was replaced");
            }
            if let Err(e) = self.persist(&n) {
                return Some(ProposalOutcome::DurabilityFailure {
                    index: Some(idx),
                    failure: e,
                });
            }
            self.apply_ready(&mut n); // sole voter queues its committed apply here
            (idx, term, identity)
        };
        self.flush().await;

        if let Some(failure) = self.latched_failure.lock().unwrap().clone() {
            return Some(ProposalOutcome::DurabilityFailure {
                index: Some(index),
                failure,
            });
        }
        let mut rx = self.applied_tx.subscribe();
        let deadline = Instant::now() + self.cfg.propose_timeout();
        loop {
            {
                let mut n = self.node.lock().await;
                self.apply_ready(&mut n);
                if let Some(failure) = self.latched_failure.lock().unwrap().clone() {
                    return Some(ProposalOutcome::DurabilityFailure {
                        index: Some(index),
                        failure,
                    });
                }
                let entry_term = identity.committed_or_resident_term(&n);
                if let Some(replacing_term) = entry_term {
                    if replacing_term != term {
                        return Some(ProposalOutcome::Ambiguous {
                            index: Some(index),
                            reason: format!(
                                "raft: proposal at index {index} (term {term}) superseded by entry of term {replacing_term}"
                            ),
                        });
                    }
                }
                if self.completed_applied_index() >= index {
                    if entry_term == Some(term) {
                        return Some(ProposalOutcome::Completed { index });
                    }
                    return Some(ProposalOutcome::Ambiguous {
                        index: Some(index),
                        reason: format!("raft: proposal identity unavailable after compaction at index {index} (term {term})"),
                    });
                }
            }
            tokio::select! {
                _ = rx.changed() => {}
                _ = tokio::time::sleep(Duration::from_millis(5)) => {}
            }
            if Instant::now() >= deadline {
                return Some(ProposalOutcome::Ambiguous {
                    index: Some(index),
                    reason: format!("raft: apply timeout at index {index}"),
                });
            }
        }
    }
}

impl RaftHost {
    /// Propose on the leader (locally), else forward to the current leader's
    /// `/raft/publish` over h2c. Returns the assigned index once applied
    /// (read-your-write). Retries within the propose deadline while no leader.
    pub async fn propose(&self, command: Command) -> Result<Index> {
        match self.propose_outcome(command).await {
            ProposalOutcome::Completed { index } => Ok(index),
            ProposalOutcome::RejectedBeforeAdmission { reason } => {
                if let Some(backpressure) = decode_backpressure(&reason) {
                    Err(anyhow::Error::new(backpressure))
                } else {
                    Err(anyhow!(reason))
                }
            }
            ProposalOutcome::Ambiguous { reason, .. } => Err(anyhow!(reason)),
            ProposalOutcome::DurabilityFailure { failure, .. } => Err(anyhow!(failure)),
        }
    }

    /// Propose on the leader (locally), else forward to the current leader's
    /// `/raft/publish` over h2c. Returns a typed [`ProposalOutcome`]
    /// classifying the terminal state of the proposal. Retries within the
    /// propose deadline while no leader.
    pub async fn propose_outcome(&self, command: Command) -> ProposalOutcome {
        let s = &self.shared;
        if s.lifecycle_generation.load(Ordering::Acquire) > 0 {
            s.proposal_rejected_before_routing
                .fetch_add(1, Ordering::Relaxed);
            return ProposalOutcome::RejectedBeforeAdmission {
                reason: "raft: proposal admission closed".to_string(),
            };
        }
        if let Some(err) = s.latched_failure.lock().unwrap().clone() {
            return ProposalOutcome::DurabilityFailure {
                index: None,
                failure: err,
            };
        }
        let deadline = Instant::now() + s.cfg.propose_timeout();
        let mut last_route_error = None;
        loop {
            let route = {
                let n = s.node.lock().await;
                if n.is_leader() {
                    Route::Local
                } else {
                    match s.leader_url(&n).1 {
                        Some(url) => Route::Remote(url),
                        None => Route::Unknown,
                    }
                }
            };
            match route {
                Route::Local => {
                    if let Some(outcome) = s.try_propose_applied(command.clone()).await {
                        return outcome;
                    }
                }
                Route::Remote(url) => match self.forward(&url, &command).await {
                    ProposalOutcome::Completed { index } => {
                        return ProposalOutcome::Completed { index }
                    }
                    ProposalOutcome::Ambiguous {
                        index: Some(seq),
                        reason,
                    } => {
                        return ProposalOutcome::Ambiguous {
                            index: Some(seq),
                            reason,
                        };
                    }
                    outcome @ ProposalOutcome::DurabilityFailure { .. } => return outcome,
                    outcome @ ProposalOutcome::RejectedBeforeAdmission { .. } => return outcome,
                    ProposalOutcome::Ambiguous {
                        index: None,
                        reason,
                    } => {
                        last_route_error = Some(reason);
                    }
                },
                Route::Unknown => {}
            }
            if Instant::now() >= deadline {
                return match last_route_error {
                    Some(error) => ProposalOutcome::Ambiguous {
                        index: None,
                        reason: format!("raft: proposal routing timed out: {error}"),
                    },
                    None => ProposalOutcome::RejectedBeforeAdmission {
                        reason: "raft: no leader elected (cluster not ready)".to_string(),
                    },
                };
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Forward a command to the leader and wait until **this node's** state
    /// machine applies the returned index (read-your-write on a follower).
    pub(super) async fn forward(&self, leader_url: &str, command: &[u8]) -> ProposalOutcome {
        let seq = match self.shared.peer_client.forward(leader_url, command).await {
            ForwardReply::Accepted { seq } => Index::new(seq),
            ForwardReply::Backpressure {
                reason,
                retry_after_seconds,
            } => {
                return ProposalOutcome::RejectedBeforeAdmission {
                    reason: encode_backpressure(&ProposalBackpressure {
                        reason,
                        retry_after_seconds,
                    }),
                };
            }
            ForwardReply::Rejected { reason } => {
                return ProposalOutcome::RejectedBeforeAdmission { reason };
            }
            ForwardReply::Failed { reason } => {
                return ProposalOutcome::Ambiguous {
                    index: None,
                    reason,
                };
            }
        };
        // Wait for our own apply (the leader's commit propagates via AppendEntries).
        let mut rx = self.shared.applied_tx.subscribe();
        let deadline = Instant::now() + self.shared.cfg.propose_timeout();
        while self.shared.completed_applied_index() < seq {
            tokio::select! {
                _ = rx.changed() => {}
                _ = tokio::time::sleep(Duration::from_millis(5)) => {}
            }
            if Instant::now() >= deadline {
                return ProposalOutcome::Ambiguous {
                    index: Some(seq),
                    reason: format!("raft: follower apply timeout at index {seq}"),
                };
            }
        }
        ProposalOutcome::Completed { index: seq }
    }

    /// The leader-side write target. The direct router, registry router, and
    /// follower forward client all use this one public path.
    pub const PUBLISH_PATH: &'static str = PUBLISH_PATH;
}

enum Route {
    Local,
    Remote(String),
    Unknown,
}
