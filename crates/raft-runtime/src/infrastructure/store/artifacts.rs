use super::*;

impl RaftStore {
    /// Returns the snapshot artifact path for a given generation.
    pub fn artifact_path(&self, snapshot_index: Index, snapshot_term: Term) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("raft");
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        dir.join(format!(
            "{stem}-snap-{snapshot_index}-{snapshot_term}.artifact"
        ))
    }

    pub(super) fn log_artifact_path(&self, generation: &[u8; 32]) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("raft");
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        dir.join(format!("{stem}-log-{}.artifact", hex_digest(generation)))
    }

    pub(super) fn collect_superseded_artifacts(
        &self,
        current_artifact: Option<&Path>,
    ) -> io::Result<()> {
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("raft");
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = format!("{stem}-snap-");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(&prefix) && name.ends_with(".artifact") {
                        if current_artifact.map_or(true, |cur| cur != path.as_path()) {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn collect_superseded_log_artifacts(
        &self,
        current_artifact: Option<&Path>,
    ) -> io::Result<()> {
        let stem = self
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("raft");
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = format!("{stem}-log-");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
                    if name.starts_with(&prefix)
                        && name.ends_with(".artifact")
                        && current_artifact.map_or(true, |current| current != path.as_path())
                        && !self.log_artifact_is_pinned(&path)
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        Ok(())
    }

    fn log_artifact_is_pinned(&self, path: &Path) -> bool {
        self.pinned_log_generations
            .lock()
            .expect("pinned log generations mutex poisoned")
            .keys()
            .any(|generation| self.log_artifact_path(generation) == path)
    }

    pub(super) fn read_snapshot_artifact(
        &self,
        snapshot_index: Index,
        snapshot_term: Term,
        snapshot_len: usize,
        snapshot_digest: &[u8],
    ) -> io::Result<Vec<u8>> {
        if snapshot_len == 0 {
            return Ok(Vec::new());
        }
        let art_path = self.artifact_path(snapshot_index, snapshot_term);
        if !art_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "missing snapshot artifact for index {snapshot_index} term {snapshot_term}"
                ),
            ));
        }
        let art_bytes = std::fs::read(&art_path)?;
        if art_bytes.len() != snapshot_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "snapshot artifact truncated: expected {snapshot_len} bytes, found {}",
                    art_bytes.len()
                ),
            ));
        }
        let actual_digest: [u8; 32] = Sha256::digest(&art_bytes).into();
        if actual_digest.as_slice() != snapshot_digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot artifact digest mismatch (content corrupted)",
            ));
        }
        Ok(art_bytes)
    }
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("write digest into String");
    }
    value
}
