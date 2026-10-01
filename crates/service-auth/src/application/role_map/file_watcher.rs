use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::reloadable::ReloadableRoleMapVerifier;

/// Production poll cadence for a Secret/CSI-projected token registry. The
/// source file is tiny, and reading it only occurs on this background cadence;
/// requests continue against the last validated in-memory snapshot.
pub const DEFAULT_REGISTRY_FILE_WATCH_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, PartialEq, Eq)]
enum RegistryFileState {
    Bytes(Vec<u8>),
    Unavailable,
}

fn read_registry_file_state(path: &Path) -> RegistryFileState {
    std::fs::read(path)
        .map(RegistryFileState::Bytes)
        .unwrap_or(RegistryFileState::Unavailable)
}

/// Spawn a background watcher for a Secret/CSI-projected registry file using
/// the production cadence. A changed file is fully parsed and validated
/// before [`ReloadableRoleMapVerifier`] adopts it, so invalid rotations retain
/// the previous known-good snapshot. The watcher never logs credential bytes.
pub fn spawn_registry_file_watcher(
    verifier: Arc<ReloadableRoleMapVerifier>,
    path: impl AsRef<Path>,
) -> tokio::task::JoinHandle<()> {
    spawn_registry_file_watcher_with_interval(verifier, path, DEFAULT_REGISTRY_FILE_WATCH_INTERVAL)
}

/// Spawn a registry watcher with an explicit polling cadence. This is public
/// for services with an intentional cadence and for deterministic tests; most
/// services should use [`spawn_registry_file_watcher`].
pub fn spawn_registry_file_watcher_with_interval(
    verifier: Arc<ReloadableRoleMapVerifier>,
    path: impl AsRef<Path>,
    poll_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    spawn_registry_files_watcher_with_interval(verifier, &[path.as_ref().to_owned()], poll_interval)
}

/// Spawn a watcher over every file a service's registry is projected from,
/// using the production cadence.
///
/// The multi-path form exists because the two namespaces can arrive from
/// different Kubernetes objects — an `identities` ConfigMap and a `tokens`
/// Secret (#2764). Reloading only the file that changed would drop the other
/// half of the registry, so a change to *any* watched file re-reads and
/// re-merges *all* of them, and the merged result is adopted as one snapshot.
pub fn spawn_registry_files_watcher(
    verifier: Arc<ReloadableRoleMapVerifier>,
    paths: &[PathBuf],
) -> tokio::task::JoinHandle<()> {
    spawn_registry_files_watcher_with_interval(
        verifier,
        paths,
        DEFAULT_REGISTRY_FILE_WATCH_INTERVAL,
    )
}

/// [`spawn_registry_files_watcher`] with an explicit polling cadence.
pub fn spawn_registry_files_watcher_with_interval(
    verifier: Arc<ReloadableRoleMapVerifier>,
    paths: &[PathBuf],
    poll_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let paths: Vec<PathBuf> = paths.to_vec();
    let poll_interval = if poll_interval.is_zero() {
        Duration::from_secs(1)
    } else {
        poll_interval
    };
    fn read_all(paths: &[PathBuf]) -> Vec<RegistryFileState> {
        paths.iter().map(|p| read_registry_file_state(p)).collect()
    }
    let initial = read_all(&paths);

    tokio::spawn(async move {
        let mut observed = initial;
        let mut ticker = tokio::time::interval(poll_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let current = read_all(&paths);
            if current == observed {
                continue;
            }
            observed = current;
            if verifier.reload_files(&paths).is_err() {
                let named = paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                tracing::warn!(
                    target: "service_auth.audit",
                    paths = %named,
                    "credential registry update rejected; retaining last known-good snapshot"
                );
            }
        }
    })
}
