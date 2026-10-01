use std::io;
use std::path::{Path, PathBuf};

use super::name::GenerationName;
use super::store::GenerationStore;
use super::tree::path_exists;

/// A caller-owned staging directory. It is consumed by `commit`.
///
/// The caller must close every writer, stop changing this directory, and stop
/// writes through any external hard link before it calls
/// [`GenerationStore::commit`]. The store validates and syncs a quiescent tree.
/// It does not coordinate handles retained by the caller.
#[derive(Debug)]
pub struct StagedGeneration {
    pub(super) generation: GenerationName,
    pub(super) path: PathBuf,
    pub(super) root: PathBuf,
}

impl StagedGeneration {
    pub fn generation(&self) -> &GenerationName {
        &self.generation
    }

    /// Return the directory that the caller may populate before `commit`.
    ///
    /// Stop all changes and close every writer before passing this value to
    /// [`GenerationStore::commit`].
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl GenerationStore {
    /// Create one unique direct-child staging directory.
    pub fn begin(&self, generation: GenerationName) -> io::Result<StagedGeneration> {
        let _guard = self.lock_commit()?;
        self.begin_locked(generation)
    }

    pub(super) fn begin_locked(&self, generation: GenerationName) -> io::Result<StagedGeneration> {
        let target = self.generation_path(&generation);
        if path_exists(&target)? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("generation already exists: {}", target.display()),
            ));
        }
        let stage = self.stage_path(&generation);
        std::fs::create_dir(&stage)?;
        Ok(StagedGeneration {
            generation,
            path: stage,
            root: self.inner.root.clone(),
        })
    }

    pub(super) fn stage_path(&self, generation: &GenerationName) -> PathBuf {
        self.inner
            .root
            .join(format!(".stage-{}", generation.as_str()))
    }
}
