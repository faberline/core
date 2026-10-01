use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use super::file_change_kind::FileChangeKind;

// ============================================================================
// DirtyFileTracker
// ============================================================================

/// Tracks which source files have changed since the last analysis flush.
///
/// Files are accumulated with `mark_dirty()` and consumed atomically with
/// `drain()`, which also resets the internal state for the next cycle.
#[derive(Debug, Default)]
pub struct DirtyFileTracker {
    /// Dirty files mapped to their most recent change kind.
    dirty: HashMap<PathBuf, FileChangeKind>,
    /// Timestamp of the last `drain()` call.
    last_drain: Option<Instant>,
}

impl DirtyFileTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a file as dirty with the given change kind.
    ///
    /// If the file was already dirty, the change kind is updated — a `Deleted`
    /// event always wins over `Modified`, and `Modified` wins over `Created`
    /// when the same file appears multiple times in rapid succession.
    pub fn mark_dirty(&mut self, path: PathBuf, kind: FileChangeKind) {
        let entry = self.dirty.entry(path).or_insert(kind);
        // Merge: Deleted > Modified > Created
        *entry = match (*entry, kind) {
            (_, FileChangeKind::Deleted) => FileChangeKind::Deleted,
            (FileChangeKind::Deleted, _) => FileChangeKind::Deleted,
            (FileChangeKind::Created, FileChangeKind::Modified) => FileChangeKind::Modified,
            (existing, _) => existing,
        };
    }

    /// Return `true` when at least one file is currently dirty.
    pub fn has_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Atomically consume all dirty entries and return them.
    ///
    /// The internal set is cleared after the call, ready for the next cycle.
    pub fn drain(&mut self) -> HashMap<PathBuf, FileChangeKind> {
        self.last_drain = Some(Instant::now());
        std::mem::take(&mut self.dirty)
    }

    /// Peek at the current dirty set without consuming it.
    pub fn dirty_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.dirty.keys()
    }

    /// Number of currently dirty files.
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }
}
