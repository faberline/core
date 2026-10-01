//! Incremental daemon update manager (R6)
//!
//! Replaces the former full-workspace re-indexing triggered on every file-
//! change event with a two-phase incremental pipeline:
//!
//! 1. **Dirty file tracking** — `DirtyFileTracker` records which files have
//!    changed (created / modified / deleted) since the last analysis pass.
//!
//! 2. **Dependency-aware invalidation** — `DependencyGraph` tracks the
//!    importer ↔ dependency relationship between files so that, when a module
//!    changes, every file that imports it is also marked dirty.
//!
//! 3. **Incremental update manager** — `IncrementalUpdateManager` wires the
//!    two components together and exposes a single `drain_dirty_files()` method
//!    that the daemon calls on each watch-bridge event instead of the previous
//!    full re-index.
//!
//! # Integration with the daemon
//!
//! ```no_run
//! use compass::server::incremental::{FileChangeKind, IncrementalUpdateManager};
//! use std::path::PathBuf;
//!
//! let mut manager = IncrementalUpdateManager::new();
//!
//! // Record watch events
//! manager.file_changed(PathBuf::from("src/lib.rs"), FileChangeKind::Modified);
//! manager.file_changed(PathBuf::from("src/foo.rs"), FileChangeKind::Created);
//!
//! // Register an import edge: src/main.rs imports src/lib.rs
//! manager.add_import_edge(PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs"));
//!
//! // Obtain the minimal set of files that need re-analysis (transitive)
//! let to_reanalyze = manager.drain_dirty_files();
//! ```

pub use crate::domain::incremental::dependency_graph::DependencyGraph;
pub use crate::domain::incremental::dirty_file_tracker::DirtyFileTracker;
pub use crate::domain::incremental::file_change_kind::FileChangeKind;
pub use crate::domain::incremental::update_manager::IncrementalUpdateManager;
