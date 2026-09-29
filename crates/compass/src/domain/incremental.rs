//! Incremental re-analysis: dirty files, the import graph and the update manager.
pub(crate) mod dependency_graph;
pub(crate) mod dirty_file_tracker;
pub(crate) mod file_change_kind;
pub(crate) mod update_manager;
