use std::path::PathBuf;
use std::time::Duration;

/// Daemon configuration
///
/// `DaemonConfig::new` and `DaemonConfig::default_socket_path` resolve the
/// socket under the project's index directory; they live with the index
/// storage in infrastructure/daemon/socket_path.rs.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Root directory to analyze
    pub root: PathBuf,
    /// Unix socket path
    pub socket_path: PathBuf,
    /// Enable file watching
    pub watch: bool,
    /// Watch debounce duration
    pub debounce: Duration,
}

impl DaemonConfig {
    /// Set custom socket path
    pub fn with_socket(mut self, path: PathBuf) -> Self {
        self.socket_path = path;
        self
    }

    /// Enable/disable file watching
    pub fn with_watch(mut self, enabled: bool) -> Self {
        self.watch = enabled;
        self
    }
}
