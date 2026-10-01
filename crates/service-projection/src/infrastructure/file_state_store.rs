use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;

use super::file_mode::set_directory_mode;
use super::quarantine::quarantine_invalid_snapshot;
use super::state_store::{decode_snapshot, persist};
use crate::domain::{
    ProjectionCheckpoint, ProjectionDescriptor, ProjectionError, ProjectionStateStore,
};

/// The file adapter of `ProjectionStateStore`: the state of projection
/// `<name>` is `indexes/<name>/state.json` under the registry root.
pub(crate) struct FileProjectionStateStore {
    root: PathBuf,
}

impl FileProjectionStateStore {
    /// A store that keeps state in the `indexes` directory of `registry_root`.
    pub(crate) fn new(registry_root: &Path) -> Self {
        Self {
            root: registry_root.join("indexes"),
        }
    }

    fn state_path(&self, name: &str) -> PathBuf {
        self.root.join(name).join("state.json")
    }
}

impl ProjectionStateStore for FileProjectionStateStore {
    fn prepare_root(&self) -> Result<(), ProjectionError> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create projection state root {}", self.root.display()))
            .map_err(ProjectionError::other)?;
        set_directory_mode(&self.root).map_err(ProjectionError::other)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ProjectionError> {
        let path = self.state_path(name);
        if !path.exists() {
            return Ok(None);
        }
        fs::read(&path)
            .map(Some)
            .with_context(|| format!("read projection state {}", path.display()))
            .map_err(ProjectionError::other)
    }

    fn restore(
        &self,
        descriptor: &ProjectionDescriptor,
        bytes: &[u8],
    ) -> Result<(ProjectionCheckpoint, Vec<u8>), ProjectionError> {
        decode_snapshot(descriptor, bytes).map_err(ProjectionError::other)
    }

    fn quarantine(&self, name: &str, bytes: &[u8]) -> Result<(), ProjectionError> {
        quarantine_invalid_snapshot(&self.state_path(name), bytes)
            .map(drop)
            .map_err(ProjectionError::other)
    }

    fn persist(
        &self,
        name: &str,
        checkpoint: &ProjectionCheckpoint,
        state: &[u8],
    ) -> Result<(), ProjectionError> {
        persist(&self.state_path(name), checkpoint, state).map_err(ProjectionError::other)
    }
}

#[cfg(test)]
mod tests;
