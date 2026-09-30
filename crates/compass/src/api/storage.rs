//! Persistent index storage path resolution.
//!
//! Resolves the persistent storage directory for Lens code indexes at
//! `{project_dir}/cclab/.index/`. Indexes are stored locally within each
//! project for portability and easy cleanup.

pub use crate::infrastructure::index_storage::paths::{
    resolve_cache_dir, resolve_lens_storage, resolve_module_index, resolve_pid_file,
    resolve_scope_cache_dir, resolve_socket_path,
};
