use std::path::{Path, PathBuf};

use crate::domain::check::source_walker::SourceWalker;

/// The file-system [`SourceWalker`]: walks directories with jwalk and reads
/// files with `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FsSourceWalker;

impl SourceWalker for FsSourceWalker {
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        use jwalk::WalkDir;

        WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path())
            .collect()
    }

    fn read_source(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}
