//! Versioned backup set manifest with deterministic validation and canonical JSON serialization.

use super::BackupSetError;
use std::collections::HashSet;

/// Maximum size in bytes of a serialized backup set manifest (1 MiB).
pub const MAX_MANIFEST_BYTES: usize = 1_048_576;

/// Maximum number of artifacts in a single backup set.
pub const MAX_ARTIFACTS: usize = 1024;

/// Maximum length in bytes of an artifact name.
pub const MAX_ARTIFACT_NAME_BYTES: usize = 256;

/// Version 1 backup set manifest describing a collection of named backup artifacts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupSetV1 {
    /// Manifest schema version (must be 1).
    pub version: u32,
    /// Stable identifier for this backup set.
    pub set_id: String,
    /// UTC timestamp when this backup set was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Identifier of the service or component that produced this backup.
    pub producer: String,
    /// Named artifacts contained within this backup set.
    pub artifacts: Vec<BackupArtifact>,
    /// Service-specific opaque metadata object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_metadata: Option<serde_json::Value>,
}

/// A single named artifact within a [`BackupSetV1`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupArtifact {
    /// Relative path name of the artifact.
    pub name: String,
    /// Exact size of the artifact in bytes.
    pub size_bytes: u64,
    /// Digest string in the format `"sha256:"` + 64 lowercase hex characters.
    pub digest: String,
    /// MIME content type of the artifact (non-empty).
    pub content_type: String,
}

impl BackupSetV1 {
    /// Validates this backup set against all integrity, size, and naming rules.
    pub fn validate(&self) -> Result<(), BackupSetError> {
        if self.version != 1 {
            return Err(BackupSetError::UnsupportedVersion(self.version));
        }

        if !is_valid_identifier(&self.set_id) {
            return Err(BackupSetError::InvalidSetId(self.set_id.clone()));
        }

        if !is_valid_identifier(&self.producer) {
            return Err(BackupSetError::InvalidProducer(self.producer.clone()));
        }

        if self.artifacts.is_empty() {
            return Err(BackupSetError::EmptyArtifacts);
        }

        if self.artifacts.len() > MAX_ARTIFACTS {
            return Err(BackupSetError::TooManyArtifacts {
                count: self.artifacts.len(),
                max: MAX_ARTIFACTS,
            });
        }

        let mut seen_names = HashSet::with_capacity(self.artifacts.len());
        for artifact in &self.artifacts {
            if !is_valid_artifact_name(&artifact.name) {
                return Err(BackupSetError::InvalidArtifactName(artifact.name.clone()));
            }

            if !seen_names.insert(&artifact.name) {
                return Err(BackupSetError::DuplicateArtifactName(artifact.name.clone()));
            }

            if !is_valid_digest(&artifact.digest) {
                return Err(BackupSetError::InvalidDigest(artifact.digest.clone()));
            }

            if artifact.content_type.is_empty() {
                return Err(BackupSetError::EmptyContentType);
            }
        }

        if let Some(ref meta) = self.service_metadata {
            if !meta.is_object() {
                return Err(BackupSetError::InvalidServiceMetadata);
            }
        }

        Ok(())
    }

    /// Parses and validates a [`BackupSetV1`] from raw JSON bytes.
    ///
    /// Refuses input exceeding [`MAX_MANIFEST_BYTES`] before parsing,
    /// rejects unknown fields and unsupported versions, and validates all invariants.
    pub fn from_json(bytes: &[u8]) -> Result<Self, BackupSetError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(BackupSetError::ManifestTooLarge {
                size: bytes.len(),
                max: MAX_MANIFEST_BYTES,
            });
        }

        let set: Self =
            serde_json::from_slice(bytes).map_err(|e| BackupSetError::Json(e.to_string()))?;
        set.validate()?;
        Ok(set)
    }

    /// Serializes this backup set to canonical compact JSON bytes.
    ///
    /// Validates the set first, sorts artifacts by name, and sorts the keys
    /// of `serviceMetadata` at every depth.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, BackupSetError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        if let Some(ref meta) = canonical.service_metadata {
            canonical.service_metadata = Some(sort_json_value(meta));
        }
        serde_json::to_vec(&canonical).map_err(|e| BackupSetError::Json(e.to_string()))
    }
}

fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn is_valid_artifact_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_ARTIFACT_NAME_BYTES {
        return false;
    }
    if name.starts_with('/') || name.ends_with('/') {
        return false;
    }
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return false;
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return false;
        }
    }
    true
}

fn is_valid_digest(digest: &str) -> bool {
    if !digest.starts_with("sha256:") {
        return false;
    }
    let hex = &digest["sha256:".len()..];
    if hex.len() != 64 {
        return false;
    }
    hex.chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn sort_json_value(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by_key(|(k1, _)| *k1);
            let mut sorted_map = serde_json::Map::new();
            for (k, v) in entries {
                sorted_map.insert(k.clone(), sort_json_value(v));
            }
            serde_json::Value::Object(sorted_map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json_value).collect())
        }
        other => other.clone(),
    }
}
