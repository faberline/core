use std::path::PathBuf;
use std::time::Duration;

use crate::domain::daemon::config::DaemonConfig;
use crate::infrastructure::index_storage::paths::resolve_socket_path;

impl DaemonConfig {
    /// Create config with default socket path based on workspace hash
    pub fn new(root: PathBuf) -> Self {
        let socket_path = Self::default_socket_path(&root);
        Self {
            root,
            socket_path,
            watch: true,
            debounce: Duration::from_millis(300),
        }
    }

    /// Generate default socket path from workspace root.
    ///
    /// Uses `{root}/cclab/.index/daemon.sock` for local project storage.
    /// Falls back to `/tmp/cclab_lens-{hash}.sock` if resolution fails.
    pub fn default_socket_path(root: &PathBuf) -> PathBuf {
        resolve_socket_path(root).unwrap_or_else(|_| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            root.hash(&mut hasher);
            PathBuf::from(format!("/tmp/cclab_lens-{:x}.sock", hasher.finish()))
        })
    }
}

#[cfg(test)]
mod tests;
