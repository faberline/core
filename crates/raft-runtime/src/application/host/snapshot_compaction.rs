use super::*;

pub(super) async fn preflight_snapshot_with_serial(
    sm: Arc<dyn RaftStateMachine>,
    snapshot_install: Arc<Mutex<()>>,
    operation: Arc<RpcGuard>,
) -> Result<(Option<Box<dyn SnapshotPreparation>>, OwnedMutexGuard<()>)> {
    loop {
        let sm = Arc::clone(&sm);
        let preflight_operation = Arc::clone(&operation);
        let preparation = tokio::task::spawn_blocking(move || {
            // The caller can be cancelled while a product waits for checkpoint
            // capacity. Keep the drain lease until that worker releases it.
            let _operation = preflight_operation;
            sm.preflight_snapshot()
                .map_err(StateMachineError::into_anyhow)
        })
        .await??;
        match Arc::clone(&snapshot_install).try_lock_owned() {
            Ok(state_machine_lease) => return Ok((preparation, state_machine_lease)),
            Err(_) => {
                // A preparation can own a product save permit. Do not retain
                // it while an incoming restore owns this serial lease.
                drop(preparation);
                let state_machine_lease = Arc::clone(&snapshot_install).lock_owned().await;
                drop(state_machine_lease);
            }
        }
    }
}

impl RaftHost {
    /// Capture a state-machine snapshot and compact the log up to the applied
    /// index (for `SnapshotPolicy::External` consumers driving their own cadence).
    pub async fn snapshot_and_compact(&self) -> Result<Index> {
        Ok(self
            .snapshot_and_compact_with_policy(None, true)
            .await?
            .snapshot_index)
    }

    /// Return the durable local snapshot index without changing Raft state.
    pub async fn snapshot_index(&self) -> Index {
        self.shared.node.lock().await.snapshot_index()
    }

    /// Capture a product-defined checkpoint and compact only the requested
    /// applied prefix.
    ///
    /// This is for durable data-plane products that can prove an older Raft
    /// prefix is already recoverable from their own committed storage. The
    /// state machine receives the exact prefix through `snapshot_at` and can
    /// refuse when that proof is not available.
    pub async fn snapshot_and_compact_through(&self, up_to: Index) -> Result<Index> {
        Ok(self
            .snapshot_and_compact_through_outcome(up_to)
            .await?
            .snapshot_index)
    }

    /// Coordinate a product snapshot and report whether voters installed new
    /// bytes. `installed` is false when `up_to` was already compacted.
    pub async fn snapshot_and_compact_through_outcome(
        &self,
        up_to: Index,
    ) -> Result<SnapshotCompactionOutcome> {
        self.snapshot_and_compact_with_policy(Some(up_to), true)
            .await
    }

    /// Compact after a voting quorum installs a self-contained snapshot.
    ///
    /// The caller must keep every external object referenced by the snapshot
    /// until lagging voters have installed it. This method bounds the leader's
    /// resident log during one voter outage, but it does not authorize product
    /// data or archive garbage collection.
    pub async fn snapshot_and_compact_through_quorum_outcome(
        &self,
        up_to: Index,
    ) -> Result<SnapshotCompactionOutcome> {
        self.snapshot_and_compact_with_policy(Some(up_to), false)
            .await
    }

    async fn snapshot_and_compact_with_policy(
        &self,
        requested_index: Option<Index>,
        require_every_voter: bool,
    ) -> Result<SnapshotCompactionOutcome> {
        let operation = self.shared.begin_coordinated_peer_work()?;
        let (preparation, state_machine_lease) = preflight_snapshot_with_serial(
            Arc::clone(&self.shared.sm),
            Arc::clone(&self.shared.snapshot_install),
            Arc::clone(&operation),
        )
        .await?;
        let mut state_machine_lease = Some(state_machine_lease);
        let (term, snapshot_term, voters, checkpoint_index, existing_bytes) = {
            let n = self.shared.node.lock().await;
            if let Some(failure) = self.shared.latched_failure.lock().unwrap().clone() {
                return Err(failure.into());
            }
            let applied = self.shared.completed_applied_index();
            let up_to = requested_index.unwrap_or(applied);
            if up_to == 0 {
                return Ok(SnapshotCompactionOutcome {
                    snapshot_index: n.snapshot_index(),
                    installed: false,
                });
            }
            if up_to > applied {
                return Err(anyhow!(
                    "cannot compact unapplied Raft prefix {up_to}; state machine applied index is {applied}"
                ));
            }
            if up_to <= n.snapshot_index()
                && (!require_every_voter || n.conf_state().membership.voters().len() <= 1)
            {
                return Ok(SnapshotCompactionOutcome {
                    snapshot_index: n.snapshot_index(),
                    installed: false,
                });
            }
            if !n.is_leader() && n.conf_state().membership.voters().len() > 1 {
                return Err(anyhow!(
                    "only the Raft leader can coordinate voter compaction"
                ));
            }
            if up_to <= n.snapshot_index() {
                drop(state_machine_lease.take());
                let persisted = n.persisted_ref();
                (
                    n.current_term(),
                    persisted.snapshot_term,
                    n.conf_state().membership.voters().to_vec(),
                    persisted.snapshot_index,
                    Some(persisted.snapshot.to_vec()),
                )
            } else {
                let snapshot_term = n.term_at_index(up_to).ok_or_else(|| {
                    anyhow!("Raft prefix {up_to} has no term and cannot be compacted")
                })?;
                let term = n.current_term();
                let voters = n.conf_state().membership.voters().to_vec();
                drop(n);
                (term, snapshot_term, voters, up_to, None)
            }
        };
        let up_to = checkpoint_index;
        let already_compacted = existing_bytes.is_some();
        let bytes = if let Some(bytes) = existing_bytes {
            // A prepared product handle can retain a save permit. There is no
            // new capture to make after loading persisted bytes, so release it
            // before status checks and snapshot resend work begin.
            drop(preparation);
            bytes
        } else if let Some(preparation) = preparation {
            let capture_operation = Arc::clone(&operation);
            let prepared = tokio::task::spawn_blocking(move || {
                let _operation = capture_operation;
                let _serial = state_machine_lease
                    .take()
                    .expect("new snapshot must retain its state-machine lease");
                preparation
                    .capture_at(up_to)
                    .map_err(StateMachineError::into_anyhow)
            })
            .await??;
            let export_operation = Arc::clone(&operation);
            tokio::task::spawn_blocking(move || {
                // The immutable capture is safe to encode while ordered apply
                // proceeds. Retain peer-work ownership until output ends.
                let _operation = export_operation;
                let mut sink = ChunkSink::new(SNAPSHOT_CHUNK_SIZE);
                prepared
                    .write_to(&mut sink)
                    .map_err(StateMachineError::into_anyhow)?;
                Ok::<_, anyhow::Error>(sink.into_bytes())
            })
            .await??
        } else {
            let legacy_operation = Arc::clone(&operation);
            let sm = Arc::clone(&self.shared.sm);
            tokio::task::spawn_blocking(move || {
                let _operation = legacy_operation;
                let _serial = state_machine_lease
                    .take()
                    .expect("legacy snapshot must retain its state-machine lease");
                let mut sink = ChunkSink::new(SNAPSHOT_CHUNK_SIZE);
                sm.snapshot_at(up_to, &mut sink)
                    .map_err(StateMachineError::into_anyhow)?;
                Ok::<_, anyhow::Error>(sink.into_bytes())
            })
            .await??
        };

        let mut replies = Vec::new();
        let required_capability = self.shared.sm.snapshot_capability();
        if require_every_voter {
            if let Some(required) = required_capability {
                for voter in voters
                    .iter()
                    .copied()
                    .filter(|voter| *voter != self.shared.id)
                {
                    let status = self.shared.request_status(voter).await?;
                    if status.snapshot_capability.as_deref() != Some(required) {
                        return Err(anyhow!(
                            "raft: voter {voter} does not advertise snapshot capability {required}"
                        ));
                    }
                }
            }
        }
        let quorum = voters.len() / 2 + 1;
        let remote_voters = voters
            .into_iter()
            .filter(|voter| *voter != self.shared.id)
            .collect::<Vec<_>>();
        let required_remote_replies = if require_every_voter {
            remote_voters.len()
        } else {
            quorum.saturating_sub(1)
        };
        let mut refusals = Vec::new();
        let mut requests = JoinSet::new();
        for voter in remote_voters {
            let shared = Arc::clone(&self.shared);
            let data = bytes.clone();
            let operation = Arc::clone(&operation);
            requests.spawn(async move {
                // Cancellation of the caller only schedules JoinSet aborts.
                // Keep the drain lease until this worker actually stops too.
                let _operation = operation;
                let response = shared
                    .request_snapshot(
                        voter,
                        InstallSnapshotReq {
                            term,
                            leader: shared.id,
                            snapshot_index: up_to,
                            snapshot_term,
                            data,
                        },
                        required_capability,
                    )
                    .await;
                (voter, response)
            });
        }
        while replies.len() < required_remote_replies {
            let Some(joined) = requests.join_next().await else {
                break;
            };
            let (voter, response) = match joined {
                Ok(response) => response,
                Err(error) if require_every_voter => {
                    return Err(anyhow!("raft: snapshot worker failed: {error}"));
                }
                Err(error) => {
                    refusals.push(format!("snapshot worker: {error}"));
                    continue;
                }
            };
            let response = match response {
                Ok(response) => response,
                Err(error) if require_every_voter => return Err(error),
                Err(error) => {
                    refusals.push(format!("voter {voter}: {error}"));
                    continue;
                }
            };
            if response.term > term {
                let mut node = self.shared.node.lock().await;
                node.handle(voter, RaftMsg::InstallSnapshotResp(response.clone()));
                self.shared.persist(&node)?;
                return Err(anyhow!(
                    "raft: voter {voter} advanced the term while refusing snapshot {up_to}"
                ));
            }
            if response.term != term || !response.accepted || response.snapshot_index < up_to {
                let refusal = anyhow!(
                    "raft: voter {voter} refused snapshot {up_to} at term {term}; replied term {} accepted {} index {}",
                    response.term,
                    response.accepted,
                    response.snapshot_index
                );
                if require_every_voter {
                    return Err(refusal);
                }
                refusals.push(refusal.to_string());
                continue;
            }
            replies.push((voter, response));
        }
        requests.abort_all();

        if replies.len() + 1 < quorum {
            return Err(anyhow!(
                "raft: snapshot {up_to} reached {} of {} required voters; {}",
                replies.len() + 1,
                quorum,
                refusals.join("; ")
            ));
        }

        let mut n = self.shared.node.lock().await;
        if n.current_term() != term
            || (!n.is_leader() && n.conf_state().membership.voters().len() > 1)
        {
            return Err(anyhow!(
                "raft leadership changed while coordinating snapshot {up_to}"
            ));
        }
        for (voter, response) in replies {
            n.handle(voter, RaftMsg::InstallSnapshotResp(response));
        }
        n.compact(up_to, bytes);
        self.shared.persist(&n)?;
        Ok(SnapshotCompactionOutcome {
            snapshot_index: up_to,
            installed: !already_compacted,
        })
    }
}
