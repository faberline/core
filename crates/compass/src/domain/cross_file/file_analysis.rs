use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::cross_file::binding::TypeBinding;

/// Analysis state for a single file.
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    /// File path
    pub path: PathBuf,
    /// Symbols defined in this file
    pub symbols: HashMap<String, TypeBinding>,
    /// Imports from other files
    pub imports: Vec<ImportInfo>,
    /// Analysis complete
    pub complete: bool,
    /// Whether cross-file type propagation has completed for this file (R3).
    pub propagation_complete: bool,
}

/// Import information.
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// Module being imported
    pub module: String,
    /// Specific names imported (None = import all)
    pub names: Option<Vec<String>>,
    /// Alias (if any)
    pub alias: Option<String>,
}
