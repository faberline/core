use std::path::PathBuf;

/// Events from the watch bridge
///
/// The infrastructure watch bridge emits them; the daemon's background
/// analysis loop consumes them.
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// Files were modified and need re-analysis
    FilesChanged(Vec<PathBuf>),
    /// Watch error occurred
    Error(String),
    /// Watcher is ready
    Ready,
    /// Watcher stopped
    Stopped,
}
