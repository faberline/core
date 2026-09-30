use super::*;

impl RaftStore {
    /// Open (creating the dir if needed) the state file `raft-<node_id>.state`.
    pub fn open(dir: &str, node_id: NodeId, fsync: FsyncPolicy) -> io::Result<RaftStore> {
        Self::open_group(dir, node_id, GroupId(LEGACY_GROUP_ID.to_string()), fsync)
    }

    pub fn open_group(
        dir: &str,
        node_id: NodeId,
        group_id: GroupId,
        fsync: FsyncPolicy,
    ) -> io::Result<RaftStore> {
        let dir = PathBuf::from(dir);
        create_dir_all(&dir)?;
        if group_id.0 != LEGACY_GROUP_ID {
            let legacy_file = dir.join(format!("raft-{node_id}.state"));
            if legacy_file.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "cannot open named group {:?} for node {node_id}: legacy state file raft-{node_id}.state exists and must be explicitly migrated",
                        group_id.0
                    ),
                ));
            }
        }
        let filename = if group_id.0 == LEGACY_GROUP_ID {
            format!("raft-{node_id}.state")
        } else {
            let mut s = String::new();
            for b in group_id.0.as_bytes() {
                use std::fmt::Write;
                write!(&mut s, "{:02x}", b).unwrap();
            }
            format!("raft-{node_id}-{s}.state")
        };
        Ok(RaftStore {
            path: dir.join(filename),
            fsync,
            io_serial: Mutex::new(()),
            cache: Mutex::new(StoreCache::default()),
            pinned_log_generations: Arc::new(Mutex::new(BTreeMap::new())),
            log_bytes_appended: AtomicU64::new(0),
            log_bytes_rewritten: AtomicU64::new(0),
            injected_save_failure: Mutex::new(None),
            injected_after_artifact_failure: Mutex::new(None),
            injected_after_publish_failure: Mutex::new(None),
            #[cfg(test)]
            load_after_state_read_hook: Mutex::new(None),
        })
    }

    /// Access the file path of this store.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn persistence_stats(&self) -> PersistenceStats {
        PersistenceStats {
            log_bytes_appended: self.log_bytes_appended.load(Ordering::Relaxed),
            log_bytes_rewritten: self.log_bytes_rewritten.load(Ordering::Relaxed),
        }
    }
}
