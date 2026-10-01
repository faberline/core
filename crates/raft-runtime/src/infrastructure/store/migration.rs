use super::*;

fn migration_file_digest(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

fn copy_migration_artifact(source: &Path, target: &Path, fsync: FsyncPolicy) -> io::Result<()> {
    if target.exists() {
        if source.metadata()?.len() == target.metadata()?.len()
            && migration_file_digest(source)? == migration_file_digest(target)?
        {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "target artifact file already exists with different content: {}",
                target.display()
            ),
        ));
    }

    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid target artifact"))?;
    let temporary = target.with_file_name(format!(".{target_name}.migration.tmp"));
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let copy_result = (|| {
        let mut input = OpenOptions::new().read(true).open(source)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        io::copy(&mut input, &mut output)?;
        output.flush()?;
        if fsync != FsyncPolicy::Os {
            output.sync_all()?;
        }
        drop(output);
        std::fs::rename(&temporary, target)?;
        if fsync != FsyncPolicy::Os {
            storage_durable::sync_parent_dir(target).map_err(io::Error::other)?;
        }
        Ok(())
    })();
    if copy_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    copy_result
}

fn cleanup_migrated_source(
    legacy_path: &Path,
    artifacts: &[(PathBuf, PathBuf)],
    fsync: FsyncPolicy,
) -> io::Result<()> {
    for (source, _) in artifacts {
        match std::fs::remove_file(source) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    match std::fs::remove_file(legacy_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if fsync != FsyncPolicy::Os {
        storage_durable::sync_parent_dir(legacy_path).map_err(io::Error::other)?;
    }
    Ok(())
}

impl RaftStore {
    /// Explicitly migrate a node's legacy single-group state file and its snapshot
    /// artifacts to a named group.
    ///
    /// Copies artifacts before publishing the target hard state. The legacy
    /// files remain authoritative until the target store has loaded
    /// successfully, so an interrupted migration can be retried safely.
    pub fn migrate_legacy_to_group(
        dir: &str,
        node_id: NodeId,
        target_group: GroupId,
        fsync: FsyncPolicy,
    ) -> io::Result<RaftStore> {
        if target_group.as_str() == LEGACY_GROUP_ID {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot migrate legacy state to legacy group ID",
            ));
        }

        let dir_path = PathBuf::from(dir);
        let legacy_filename = format!("raft-{node_id}.state");
        let legacy_path = dir_path.join(&legacy_filename);
        if !legacy_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("legacy state file not found: {}", legacy_path.display()),
            ));
        }

        let mut s = String::new();
        for b in target_group.as_str().as_bytes() {
            use std::fmt::Write;
            write!(&mut s, "{:02x}", b).unwrap();
        }
        let target_filename = format!("raft-{node_id}-{s}.state");
        let target_path = dir_path.join(&target_filename);
        let legacy_store = RaftStore {
            path: legacy_path.clone(),
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
        };

        // Read the hard state before inspecting a possible target. A target
        // with identical bytes is an idempotent retry only when it also loads
        // through the target artifact namespace.
        let legacy_bytes = std::fs::read(&legacy_path)?;

        // Discover all associated snapshot and split-log artifact files. A V4
        // hard state resolves its log artifact from the state-file stem.
        let legacy_stem = format!("raft-{node_id}");
        let target_stem = format!("raft-{node_id}-{s}");
        let artifact_prefixes = [
            (
                format!("{legacy_stem}-snap-"),
                format!("{target_stem}-snap-"),
            ),
            (format!("{legacy_stem}-log-"), format!("{target_stem}-log-")),
        ];

        let mut artifacts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                    for (legacy_prefix, target_prefix) in &artifact_prefixes {
                        if fname.starts_with(legacy_prefix) && fname.ends_with(".artifact") {
                            let suffix = &fname[legacy_prefix.len()..];
                            let target_art_path = dir_path.join(format!("{target_prefix}{suffix}"));
                            artifacts.push((p.clone(), target_art_path));
                            break;
                        }
                    }
                }
            }
        }

        let target_store = RaftStore {
            path: target_path.clone(),
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
        };

        if target_path.exists() {
            if std::fs::read(&target_path)? != legacy_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "target state file already exists: {}",
                        target_path.display()
                    ),
                ));
            }
            target_store.load()?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "published Raft migration target has no hard state",
                )
            })?;
            cleanup_migrated_source(&legacy_path, &artifacts, fsync)?;
            return Ok(target_store);
        }

        // Validate the source while every legacy artifact is still present.
        legacy_store.decode_persisted_state(&legacy_bytes)?;
        for (source, target) in &artifacts {
            copy_migration_artifact(source, target, fsync)?;
        }
        storage_durable::atomic_write(&target_path, &legacy_bytes, fsync)
            .map_err(io::Error::other)?;
        target_store.load()?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "published Raft migration target has no hard state",
            )
        })?;
        cleanup_migrated_source(&legacy_path, &artifacts, fsync)?;
        Ok(target_store)
    }
}
