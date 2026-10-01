//! Import resolution for cross-file type checking
//!
//! This module handles:
//! - Parsing import statements
//! - Resolving module paths to .py and .pyi files
//! - Loading exported types from other modules
//! - Module indexing for quick lookup
//! - Lazy loading with caching to minimize memory usage
//! - Circular import detection and handling

pub use crate::domain::modules::import::{
    parse_import, Import, ImportedName, ModuleIndexEntry, ModuleInfo, ModuleLoadState,
};
pub use crate::infrastructure::import_resolution::resolver::ImportResolver;
