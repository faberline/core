// ============================================================================
// FileChangeKind
// ============================================================================

/// The type of file-system event that made a file dirty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    /// The file was newly created.
    Created,
    /// The file was modified (content change).
    Modified,
    /// The file was deleted.
    Deleted,
}
