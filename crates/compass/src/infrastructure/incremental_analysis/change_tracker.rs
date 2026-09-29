use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::domain::module_cache::content_hash::ContentHash;

// ============================================================================
// Change Tracking
// ============================================================================

/// A change to a file.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// File that changed
    pub file: PathBuf,
    /// Type of change
    pub kind: ChangeKind,
    /// Content hash after change
    pub new_hash: ContentHash,
    /// Timestamp of change
    pub timestamp: Instant,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// File created
    Created,
    /// File modified
    Modified,
    /// File deleted
    Deleted,
    /// File renamed (old path stored separately)
    Renamed,
}

/// Tracks changes across files.
pub struct ChangeTracker {
    /// Pending changes
    pending: Vec<FileChange>,
    /// Last known hash per file
    file_hashes: HashMap<PathBuf, ContentHash>,
    /// Debounce duration
    debounce: Duration,
}

impl ChangeTracker {
    /// Create a new change tracker.
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            file_hashes: HashMap::new(),
            debounce: Duration::from_millis(300),
        }
    }

    /// Set debounce duration.
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    /// Record a file change.
    pub fn record_change(&mut self, file: PathBuf, kind: ChangeKind, new_hash: ContentHash) {
        self.pending.push(FileChange {
            file: file.clone(),
            kind,
            new_hash: new_hash.clone(),
            timestamp: Instant::now(),
        });

        if kind != ChangeKind::Deleted {
            self.file_hashes.insert(file, new_hash);
        } else {
            self.file_hashes.remove(&file);
        }
    }

    /// Get pending changes (after debounce).
    pub fn get_pending_changes(&mut self) -> Vec<FileChange> {
        let now = Instant::now();
        let debounce = self.debounce;

        // Get changes that are past debounce period
        let (ready, pending): (Vec<_>, Vec<_>) = self
            .pending
            .drain(..)
            .partition(|c| now.duration_since(c.timestamp) >= debounce);

        self.pending = pending;
        ready
    }

    /// Check if a file has changed.
    pub fn has_changed(&self, file: &PathBuf, hash: &ContentHash) -> bool {
        self.file_hashes
            .get(file)
            .map(|h| h != hash)
            .unwrap_or(true)
    }

    /// Clear all pending changes.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

impl Default for ChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}
