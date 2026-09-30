//! The source-walker port: which files a check covers and their text.
//!
//! The check use case reads the file system only through this trait; the
//! implementation that walks directories and reads files is
//! `FsSourceWalker` in infrastructure/check.

use std::path::{Path, PathBuf};

/// Lists and reads the source files a check run covers.
pub trait SourceWalker {
    /// Whether `path` is an existing regular file.
    fn is_file(&self, path: &Path) -> bool;

    /// Whether `path` is an existing directory.
    fn is_dir(&self, path: &Path) -> bool;

    /// Every regular file below `dir`, recursively.
    fn files_under(&self, dir: &Path) -> Vec<PathBuf>;

    /// The text of `path`, or `None` when it cannot be read as UTF-8.
    fn read_source(&self, path: &Path) -> Option<String>;
}
