//! Role-map verifiers: the static registry verifier, the hot-reloadable one
//! with its audit sink, and the registry file watcher that feeds it.

mod file_watcher;
mod reloadable;
mod static_verifier;

pub use file_watcher::{
    spawn_registry_file_watcher, spawn_registry_file_watcher_with_interval,
    spawn_registry_files_watcher_with_interval, DEFAULT_REGISTRY_FILE_WATCH_INTERVAL,
};
pub use reloadable::ReloadableRoleMapVerifier;
pub use static_verifier::StaticRoleMapVerifier;

#[cfg(test)]
mod tests;
