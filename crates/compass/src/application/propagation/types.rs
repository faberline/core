use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Request describing which files to propagate and whether the run is
/// incremental (only changed files) or full.
#[derive(Debug, Clone)]
pub struct PropagationRequest {
    /// All files in the project that should participate in propagation.
    pub files: Vec<PathBuf>,
    /// Files that changed since the last propagation (for incremental mode).
    /// Empty means full propagation.
    pub changed_files: Vec<PathBuf>,
}

/// A single type binding that was propagated from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagatedType {
    /// Symbol name as imported.
    pub symbol: String,
    /// Resolved type signature string.
    pub type_str: String,
    /// File where symbol is originally defined.
    pub source_file: PathBuf,
    /// Line number in source file.
    pub source_line: u32,
    /// True if the type came from a `.pyi` stub.
    pub is_stub: bool,
}

/// Result of running the propagation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationResult {
    /// Map of target file → list of propagated types received.
    pub propagated: HashMap<PathBuf, Vec<PropagatedType>>,
    /// Detected import cycles (each cycle is a list of file paths).
    pub cycles: Vec<Vec<PathBuf>>,
    /// Aggregate statistics.
    pub stats: PropagationStats,
}

/// Aggregate statistics for the propagation run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropagationStats {
    pub files_analyzed: usize,
    pub symbols_propagated: usize,
    pub cycles_detected: usize,
    pub stubs_used: usize,
    pub time_ms: u64,
}
