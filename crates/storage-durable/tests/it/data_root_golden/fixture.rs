//! The policy the golden data-root tests open their roots with.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use storage_durable::DataRootPolicy;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenManifest {
    pub version: u32,
    pub role: String,
    /// Empty in every pinned layout, so it never reaches the bytes. A
    /// non-empty map cannot be encoded, because JSON keys must be strings.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unencodable: BTreeMap<Vec<u8>, u8>,
}

impl GoldenManifest {
    pub fn with_role(role: &str) -> Self {
        Self {
            version: 1,
            role: role.into(),
            unencodable: BTreeMap::new(),
        }
    }
}

/// Keeps the default `manifest_file` and `legacy_error`.
#[derive(Clone, Copy)]
pub struct GoldenPolicy {
    pub directories: &'static [&'static str],
}

impl GoldenPolicy {
    pub const fn new() -> Self {
        Self {
            directories: &["wal/logs", "tmp"],
        }
    }
}

impl DataRootPolicy for GoldenPolicy {
    type Manifest = GoldenManifest;

    fn product_name(&self) -> &'static str {
        "golden"
    }

    fn directories(&self) -> &'static [&'static str] {
        self.directories
    }

    fn legacy_markers(&self) -> &'static [&'static str] {
        &["legacy.data"]
    }

    fn create_manifest(&self, _root: &Path) -> anyhow::Result<Self::Manifest> {
        Ok(GoldenManifest::with_role("store"))
    }

    fn validate_manifest(&self, manifest: &Self::Manifest) -> anyhow::Result<()> {
        anyhow::ensure!(
            manifest.version == 1,
            "unsupported golden format {}",
            manifest.version
        );
        Ok(())
    }
}
