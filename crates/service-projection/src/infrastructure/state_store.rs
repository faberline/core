use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::file_mode::{set_directory_mode, set_file_mode};
use crate::domain::{sha256, ProjectionCheckpoint, ProjectionDescriptor};

pub const PROJECTION_STATE_FORMAT_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ProjectionStateEnvelope {
    pub format_version: u16,
    pub checkpoint: ProjectionCheckpoint,
    pub state_encoding: String,
    pub state_base64: String,
}

fn validate_envelope(
    descriptor: &ProjectionDescriptor,
    envelope: &ProjectionStateEnvelope,
) -> Result<()> {
    if envelope.format_version != PROJECTION_STATE_FORMAT_VERSION {
        bail!(
            "unsupported projection state format {}",
            envelope.format_version
        );
    }
    if envelope.checkpoint.projection != descriptor.name()
        || envelope.checkpoint.schema_version != descriptor.schema_version()
    {
        bail!("projection checkpoint descriptor does not match registered projection");
    }
    if envelope.state_encoding != "base64" {
        bail!("unsupported projection state encoding");
    }
    Ok(())
}

/// Decode a state file and check it against `descriptor`: returns the
/// checkpoint and the state bytes whose sha256 it records.
pub(super) fn decode_snapshot(
    descriptor: &ProjectionDescriptor,
    bytes: &[u8],
) -> Result<(ProjectionCheckpoint, Vec<u8>)> {
    let envelope: ProjectionStateEnvelope =
        serde_json::from_slice(bytes).context("decode projection state envelope")?;
    validate_envelope(descriptor, &envelope)?;
    let state = BASE64
        .decode(&envelope.state_base64)
        .context("decode projection state_base64")?;
    if sha256(&state) != envelope.checkpoint.state_sha256 {
        bail!(
            "projection {} state checksum does not match its checkpoint",
            descriptor.name()
        );
    }
    Ok((envelope.checkpoint, state))
}

pub(crate) fn persist(path: &Path, checkpoint: &ProjectionCheckpoint, state: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("projection state path {} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create projection directory {}", parent.display()))?;
    set_directory_mode(parent)?;
    let envelope = ProjectionStateEnvelope {
        format_version: PROJECTION_STATE_FORMAT_VERSION,
        checkpoint: checkpoint.clone(),
        state_encoding: "base64".to_string(),
        state_base64: BASE64.encode(state),
    };
    storage_durable::atomic_write(
        path,
        &serde_json::to_vec_pretty(&envelope)?,
        storage_durable::FsyncPolicy::Always,
    )
    .with_context(|| format!("atomically persist projection state {}", path.display()))?;
    set_file_mode(path)
}

#[cfg(test)]
mod tests;
