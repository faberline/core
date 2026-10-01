use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use super::current::{read_current_from, CurrentReadError, CurrentTarget};
use super::failure_injection::{FailureInjector, NoFailures};
use super::name::GenerationName;

pub(super) struct RootCommitState {
    pub(super) durable_current: Option<GenerationName>,
}

pub(super) struct GenerationStoreInner {
    pub(super) root: PathBuf,
    pub(super) injector: Arc<dyn FailureInjector>,
    pub(super) commit: Arc<Mutex<RootCommitState>>,
}

/// An opt-in durable store for immutable directory generations.
///
/// Stores opened for the same canonical root inside one process share a commit
/// mutex. A single process must own all mutations for a root. Independent
/// writers in other processes are unsupported because this API does not take
/// an operating-system file lock.
#[derive(Clone)]
pub struct GenerationStore {
    pub(super) inner: Arc<GenerationStoreInner>,
}

impl GenerationStore {
    /// Open an existing real directory with production filesystem behavior.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        Self::open_with_injector(root, Arc::new(NoFailures))
    }

    /// Open an existing real directory with deterministic failure injection.
    pub fn open_with_injector(
        root: impl Into<PathBuf>,
        injector: Arc<dyn FailureInjector>,
    ) -> io::Result<Self> {
        let root = root.into();
        let metadata = std::fs::symlink_metadata(&root)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "generation root must be a real directory: {}",
                    root.display()
                ),
            ));
        }
        let root = std::fs::canonicalize(root)?;
        let commit = shared_root_state(&root)?;
        Ok(Self {
            inner: Arc::new(GenerationStoreInner {
                root,
                injector,
                commit,
            }),
        })
    }

    /// Resolve only the exact target named by `CURRENT`.
    pub fn read_current(&self) -> Result<CurrentTarget, CurrentReadError> {
        read_current_from(&self.inner.root, |_| Ok(()))
    }

    /// Return the exact direct-child path for one generation.
    pub fn generation_path(&self, generation: &GenerationName) -> PathBuf {
        self.inner.root.join(generation.as_str())
    }

    pub(super) fn lock_commit(&self) -> io::Result<std::sync::MutexGuard<'_, RootCommitState>> {
        self.inner
            .commit
            .lock()
            .map_err(|_| io::Error::other("generation commit mutex poisoned"))
    }
}

fn shared_root_state(root: &Path) -> io::Result<Arc<Mutex<RootCommitState>>> {
    static ROOT_STATES: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<RootCommitState>>>>> =
        OnceLock::new();

    let registry = ROOT_STATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = registry
        .lock()
        .map_err(|_| io::Error::other("generation root-lock registry poisoned"))?;
    registry.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = registry.get(root).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let state = Arc::new(Mutex::new(RootCommitState {
        durable_current: None,
    }));
    registry.insert(root.to_path_buf(), Arc::downgrade(&state));
    Ok(state)
}
