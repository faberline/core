/// Atomically replace the running executable: write a sibling temp file (same
/// dir ⇒ same filesystem), make it executable, then `rename` over self. A
/// permission failure leaves the existing binary intact.
#[cfg(feature = "online")]
pub(crate) fn install_over_self(bin: &[u8], tmp_label: &str) -> anyhow::Result<()> {
    use anyhow::{anyhow, Context};
    let exe = std::env::current_exe().context("locate current executable")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("current executable has no parent directory"))?;
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

#[cfg(feature = "online")]
fn install_error(e: std::io::Error, exe: &std::path::Path) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        anyhow::anyhow!(
            "cannot replace {}: permission denied. Re-run with elevated permissions (e.g. sudo) or reinstall manually.",
            exe.display()
        )
    } else {
        anyhow::anyhow!("failed to install new binary at {}: {e}", exe.display())
    }
}
