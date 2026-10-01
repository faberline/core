use std::{
    fs::OpenOptions,
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;
use storage_durable::set_private_file_mode;

use crate::domain::QuarantineSink;

/// Append-only JSONL quarantine with an fsync before returning.
pub struct JsonlQuarantine<T> {
    path: PathBuf,
    marker: PhantomData<T>,
}

impl<T> JsonlQuarantine<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            marker: PhantomData,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<T: Serialize> QuarantineSink<T> for JsonlQuarantine<T> {
    type Error = anyhow::Error;

    fn append(&mut self, entries: &[T]) -> Result<()> {
        append_jsonl(&self.path, entries)
    }
}

pub fn append_jsonl<T: Serialize>(path: &Path, entries: &[T]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create quarantine directory {}", parent.display()))?;
        }
    }
    let mut bytes = Vec::new();
    for entry in entries {
        serde_json::to_writer(&mut bytes, entry)?;
        bytes.push(b'\n');
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open collector quarantine {}", path.display()))?;
    set_private_file_mode(path)?;
    file.write_all(&bytes)
        .with_context(|| format!("append collector quarantine {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("fsync collector quarantine {}", path.display()))
}
