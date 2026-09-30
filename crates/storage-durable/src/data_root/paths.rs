//! Filesystem checks and modes for the paths inside a data root.
//!
//! The data root uses the typed checks. The public functions keep the
//! `anyhow` errors they have always returned, with the same text and the same
//! `io::Error` to downcast to, because services call them on their own paths.

use std::{fs, io, path::Path};

use super::error::{DataRootError, IoContext};

/// Refuse `path` if it is a symlink. A path that does not exist passes.
pub fn reject_symlink(path: &Path) -> anyhow::Result<()> {
    check_not_symlink(path).map_err(DataRootError::into_anyhow)
}

/// Set `path` to mode `0700` on unix. Elsewhere this does nothing.
pub fn set_private_directory_mode(path: &Path) -> anyhow::Result<()> {
    private_directory_mode(path).map_err(DataRootError::into_anyhow)
}

/// Set `path` to mode `0600` on unix. Elsewhere this does nothing.
pub fn set_private_file_mode(path: &Path) -> anyhow::Result<()> {
    private_file_mode(path).map_err(DataRootError::into_anyhow)
}

pub(super) fn check_not_symlink(path: &Path) -> Result<(), DataRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DataRootError::Symlink {
            path: path.to_path_buf(),
        }),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).io_context(|| format!("inspect {}", path.display())),
    }
}

pub(super) fn require_directory(path: &Path) -> Result<(), DataRootError> {
    let metadata = fs::symlink_metadata(path)
        .io_context(|| format!("inspect data directory {}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(DataRootError::NotADirectory {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(super) fn require_regular_file(path: &Path) -> Result<(), DataRootError> {
    let metadata = fs::symlink_metadata(path)
        .io_context(|| format!("inspect data file {}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(DataRootError::NotARegularFile {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn private_directory_mode(path: &Path) -> Result<(), DataRootError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .io_context(|| format!("set private directory mode on {}", path.display()))
}

#[cfg(not(unix))]
pub(super) fn private_directory_mode(_path: &Path) -> Result<(), DataRootError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn private_file_mode(path: &Path) -> Result<(), DataRootError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .io_context(|| format!("set private file mode on {}", path.display()))
}

#[cfg(not(unix))]
pub(super) fn private_file_mode(_path: &Path) -> Result<(), DataRootError> {
    Ok(())
}
