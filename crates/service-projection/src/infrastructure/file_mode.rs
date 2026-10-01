#[cfg(unix)]
use std::fs;
use std::path::Path;

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;

#[cfg(unix)]
pub(crate) fn set_directory_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "set private projection directory mode on {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
pub(crate) fn set_directory_mode(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_file_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("set private projection file mode on {}", path.display()))
}

#[cfg(not(unix))]
pub(super) fn set_file_mode(_path: &Path) -> Result<()> {
    Ok(())
}
