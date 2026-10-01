use std::path::Path;

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use storage_durable::{atomic_write, set_private_file_mode, FsyncPolicy};

pub fn load_json_checkpoint<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)
        .with_context(|| format!("read collector checkpoint {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("decode collector checkpoint {}", path.display()))
        .map(Some)
}

pub fn save_json_checkpoint<T: Serialize>(path: &Path, checkpoint: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    atomic_write(path, &bytes, FsyncPolicy::Always)
        .with_context(|| format!("commit collector checkpoint {}", path.display()))?;
    set_private_file_mode(path)
}
