use std::path::Path;

use crate::domain::release::{InstallError, SelfInstall};

/// The [`SelfInstall`] port on the local filesystem.
pub(crate) struct SelfReplace;

impl SelfInstall for SelfReplace {
    /// Atomically replace the running executable: write a sibling temp file
    /// (same dir ⇒ same filesystem), make it executable, then `rename` over
    /// self. A permission failure leaves the existing binary intact.
    fn install_over_self(&self, bin: &[u8], tmp_label: &str) -> Result<(), InstallError> {
        let exe = std::env::current_exe().map_err(InstallError::Locate)?;
        let exe = exe.canonicalize().unwrap_or(exe);
        let dir = exe.parent().ok_or(InstallError::NoParent)?;
        let tmp = dir.join(format!(".{tmp_label}-{}.tmp", std::process::id()));

        let write = || -> std::io::Result<()> {
            std::fs::write(&tmp, bin)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
            }
            Ok(())
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&tmp);
            return Err(install_error(e, &exe));
        }
        if let Err(e) = std::fs::rename(&tmp, &exe) {
            let _ = std::fs::remove_file(&tmp);
            return Err(install_error(e, &exe));
        }
        Ok(())
    }
}

fn install_error(e: std::io::Error, exe: &Path) -> InstallError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        InstallError::PermissionDenied {
            exe: exe.to_path_buf(),
        }
    } else {
        InstallError::Failed {
            exe: exe.to_path_buf(),
            cause: e,
        }
    }
}
