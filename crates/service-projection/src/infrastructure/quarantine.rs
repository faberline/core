use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};

use super::file_mode::set_file_mode;
use crate::domain::sha256;

pub(crate) fn quarantine_invalid_snapshot(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("projection state path {} has no parent", path.display()))?;
    let digest = sha256(bytes);
    let prefix = &digest[..16];
    let mut target = parent.join(format!("state.corrupt-{prefix}.json"));
    for suffix in 1..=1_000_u16 {
        if !target.exists() {
            fs::rename(path, &target).with_context(|| {
                format!(
                    "quarantine invalid projection state {} as {}",
                    path.display(),
                    target.display()
                )
            })?;
            set_file_mode(&target)?;
            storage_durable::sync_parent_dir(&target)?;
            return Ok(target);
        }
        target = parent.join(format!("state.corrupt-{prefix}-{suffix}.json"));
    }
    bail!(
        "cannot allocate quarantine name for invalid projection state {}",
        path.display()
    )
}
