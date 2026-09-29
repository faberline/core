use super::*;

impl RaftStore {
    /// Durably persist the hard state (atomic temp-write + rename, fsync unless
    /// [`FsyncPolicy::Os`]).
    pub fn save(&self, state: &PersistedState) -> io::Result<()> {
        self.save_ref(&PersistedStateRef {
            term: state.term,
            voted_for: state.voted_for,
            log: &state.log,
            commit_index: state.commit_index,
            snapshot_index: state.snapshot_index,
            snapshot_term: state.snapshot_term,
            snapshot: &state.snapshot,
            conf: state.conf.as_ref(),
        })
    }

    /// Persist borrowed state without cloning the resident Raft log.
    pub fn save_ref(&self, state: &PersistedStateRef<'_>) -> io::Result<()> {
        let _io_serial = self
            .io_serial
            .lock()
            .expect("raft store I/O mutex poisoned");
        let mut cache = self.cache.lock().expect("raft store cache poisoned");
        let log_plan = plan_log_write(state, cache.log.as_ref().map(|log| log.layout));
        let next_log = log_plan.layout();
        let frame_update = command_frame_update(&log_plan)?;
        validate_command_frame_update(&cache, next_log, &frame_update)?;
        let snapshot_digest: [u8; 32] = Sha256::digest(state.snapshot).into();
        let bytes = encode_persisted_state_v4(state, &snapshot_digest, next_log);
        let digest: [u8; 32] = Sha256::digest(&bytes).into();

        if cache.hard_digest == Some(digest) {
            return Ok(());
        }

        if let Some(kind) = self
            .injected_save_failure
            .lock()
            .expect("injected_save_failure mutex poisoned")
            .take()
        {
            return Err(io::Error::new(
                kind,
                "injected save failure (fault-injection seam)",
            ));
        }

        let current_artifact = if !state.snapshot.is_empty() {
            let art_path = self.artifact_path(state.snapshot_index, state.snapshot_term);
            if !art_path.exists() {
                storage_durable::atomic_write(&art_path, state.snapshot, self.fsync)
                    .map_err(io::Error::other)?;
            }
            Some(art_path)
        } else {
            None
        };

        self.persist_log_plan(&log_plan)?;

        if let Some(kind) = self
            .injected_after_artifact_failure
            .lock()
            .expect("injected_after_artifact_failure mutex poisoned")
            .take()
        {
            return Err(io::Error::new(
                kind,
                "injected after-artifact failure (fault-injection seam)",
            ));
        }

        storage_durable::atomic_write(&self.path, &bytes, self.fsync).map_err(io::Error::other)?;

        apply_command_frame_update(&mut cache, next_log, frame_update);
        cache.hard_digest = Some(digest);
        cache.commit_index = state.commit_index;

        if let Some(kind) = self
            .injected_after_publish_failure
            .lock()
            .expect("injected_after_publish_failure mutex poisoned")
            .take()
        {
            return Err(io::Error::new(
                kind,
                "injected after-publish failure (fault-injection seam)",
            ));
        }

        let _ = self.collect_superseded_artifacts(current_artifact.as_deref());
        let current_log = next_log.map(|layout| self.log_artifact_path(&layout.generation));
        let _ = self.collect_superseded_log_artifacts(current_log.as_deref());
        Ok(())
    }

    fn persist_log_plan(&self, plan: &LogWritePlan) -> io::Result<()> {
        match plan {
            LogWritePlan::Unchanged(_) => Ok(()),
            LogWritePlan::Append {
                previous, bytes, ..
            } => {
                let path = self.log_artifact_path(&previous.generation);
                let file = OpenOptions::new().read(true).write(true).open(&path)?;
                let actual_len = file.metadata()?.len();
                if actual_len < previous.byte_len {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Raft log artifact is shorter than committed hard state",
                    ));
                }
                file.set_len(previous.byte_len)?;
                let mut file = file;
                file.seek(SeekFrom::Start(previous.byte_len))?;
                file.write_all(bytes)?;
                file.flush()?;
                if self.fsync != FsyncPolicy::Os {
                    file.sync_all()?;
                }
                self.log_bytes_appended
                    .fetch_add(bytes.len() as u64, Ordering::Relaxed);
                Ok(())
            }
            LogWritePlan::Rewrite {
                next: Some(next),
                bytes,
            } => {
                let path = self.log_artifact_path(&next.generation);
                storage_durable::atomic_write(&path, bytes, self.fsync)
                    .map_err(io::Error::other)?;
                self.log_bytes_rewritten
                    .fetch_add(bytes.len() as u64, Ordering::Relaxed);
                Ok(())
            }
            LogWritePlan::Rewrite { next: None, .. } => Ok(()),
        }
    }

    /// Seed a brand-new node with an externally supplied state-machine
    /// snapshot. The snapshot becomes the node's committed compaction point,
    /// so the next command starts at `snapshot_index + 1`. Existing hard state
    /// is never overwritten.
    pub fn seed_snapshot(
        &self,
        snapshot_index: Index,
        snapshot_term: Term,
        snapshot: Vec<u8>,
    ) -> io::Result<()> {
        if self.path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("raft state already exists at {}", self.path.display()),
            ));
        }
        self.save(&PersistedState {
            term: snapshot_term,
            voted_for: None,
            log: Vec::new(),
            commit_index: snapshot_index,
            snapshot_index,
            snapshot_term,
            snapshot,
            conf: None,
        })
    }
}
