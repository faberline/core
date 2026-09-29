use super::*;

impl RaftStore {
    /// Load the persisted hard state, or `None` if this node has none yet.
    pub fn load(&self) -> io::Result<Option<PersistedState>> {
        let _io_serial = self
            .io_serial
            .lock()
            .expect("raft store I/O mutex poisoned");
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                #[cfg(test)]
                self.wait_after_load_state_read();
                let state = self.decode_persisted_state(&bytes)?;
                let mut cache = self.cache.lock().expect("raft store cache poisoned");
                if bytes.starts_with(MAGIC_V4) {
                    cache.hard_digest = Some(Sha256::digest(&bytes).into());
                } else {
                    cache.hard_digest = None;
                    cache.log = None;
                }
                Ok(Some(state))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(super) fn read_log_artifact(
        &self,
        layout: LogLayout,
    ) -> io::Result<(Vec<RaftEntry>, BTreeMap<(Index, Term), CommandFrame>)> {
        if layout.entry_count == 0 || layout.byte_len < LOG_MAGIC_V1.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Raft log artifact layout",
            ));
        }
        let path = self.log_artifact_path(&layout.generation);
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
        let actual_len = file.metadata()?.len();
        if actual_len < layout.byte_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Raft log artifact is truncated",
            ));
        }
        let declared_len = usize::try_from(layout.byte_len).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Raft log artifact exceeds addressable memory",
            )
        })?;
        let mut bytes = vec![0_u8; declared_len];
        file.read_exact(&mut bytes)?;
        if actual_len > layout.byte_len {
            file.set_len(layout.byte_len)?;
            if self.fsync != FsyncPolicy::Os {
                file.sync_all()?;
            }
        }
        let log = decode_log_artifact(&bytes, layout)?;
        let commands = command_frames_from_artifact(&bytes, layout)?;
        Ok((log, commands))
    }
}
