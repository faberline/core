//! What `upgrade` needs from outside: the tool's GitHub releases, their
//! downloads, and the replacement of the running binary.

use std::path::PathBuf;

use super::remote::RemoteError;

/// The tool's GitHub releases and their asset downloads.
pub(crate) trait ReleaseSource {
    /// The repo's releases, as GitHub's release-list JSON (up to 100).
    async fn releases(&self, repo: &str) -> Result<serde_json::Value, RemoteError>;
    /// The release tagged `tag`, as GitHub's release JSON.
    async fn release(&self, repo: &str, tag: &str) -> Result<serde_json::Value, RemoteError>;
    /// The bytes at a download URL.
    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>, RemoteError>;
    /// The text at a download URL.
    async fn download_text(&self, url: &str) -> Result<String, RemoteError>;
}

/// Replaces the running executable with a new binary.
pub(crate) trait SelfInstall {
    /// Install `bin` over the running executable. `tmp_label` names the
    /// temporary file written next to it. On failure the existing binary
    /// stays in place.
    fn install_over_self(&self, bin: &[u8], tmp_label: &str) -> Result<(), InstallError>;
}

/// A binary that could not be installed over the running executable.
#[derive(Debug, thiserror::Error)]
pub(crate) enum InstallError {
    /// The path of the running executable is unknown.
    #[error("locate current executable")]
    Locate(#[source] std::io::Error),
    /// The running executable has no directory to write next to.
    #[error("current executable has no parent directory")]
    NoParent,
    /// The process may not write the executable's directory or file.
    #[error(
        "cannot replace {}: permission denied. Re-run with elevated permissions (e.g. sudo) or reinstall manually.",
        .exe.display()
    )]
    PermissionDenied { exe: PathBuf },
    /// Writing or renaming the new binary failed. The I/O error is part of
    /// the message, not a source.
    #[error("failed to install new binary at {}: {cause}", .exe.display())]
    Failed { exe: PathBuf, cause: std::io::Error },
}
