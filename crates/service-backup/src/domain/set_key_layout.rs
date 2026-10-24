//! Immutable, collision-safe layout of backup set keys in object storage sinks.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetKeyError {
    #[error("prefix has leading slash: {0:?}")]
    LeadingSlashInPrefix(String),
    #[error("prefix has trailing slash: {0:?}")]
    TrailingSlashInPrefix(String),
    #[error("prefix contains empty segment: {0:?}")]
    EmptyPrefixSegment(String),
    #[error("prefix segment is dot or dot-dot: {0:?}")]
    DotSegmentInPrefix(String),
    #[error("prefix segment contains invalid character: {0:?}")]
    InvalidPrefixCharacter(String),

    #[error("set id is empty")]
    EmptySetId,
    #[error("set id exceeds 128 bytes: {0}")]
    SetIdTooLong(usize),
    #[error("set id contains slash: {0:?}")]
    SlashInSetId(String),
    #[error("set id contains invalid character: {0:?}")]
    InvalidSetIdCharacter(String),

    #[error("artifact name is empty")]
    EmptyArtifactName,
    #[error("artifact name exceeds 256 bytes: {0}")]
    ArtifactNameTooLong(usize),
    #[error("artifact name has leading slash: {0:?}")]
    LeadingSlashInArtifactName(String),
    #[error("artifact name has trailing slash: {0:?}")]
    TrailingSlashInArtifactName(String),
    #[error("artifact name contains empty segment: {0:?}")]
    EmptyArtifactNameSegment(String),
    #[error("artifact name segment is dot or dot-dot: {0:?}")]
    DotSegmentInArtifactName(String),
    #[error("artifact name segment contains invalid character: {0:?}")]
    InvalidArtifactNameCharacter(String),
}

fn is_valid_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-'
}

fn validate_prefix(prefix: &str) -> Result<(), SetKeyError> {
    if prefix.is_empty() {
        return Ok(());
    }
    if prefix.starts_with('/') {
        return Err(SetKeyError::LeadingSlashInPrefix(prefix.to_string()));
    }
    if prefix.ends_with('/') {
        return Err(SetKeyError::TrailingSlashInPrefix(prefix.to_string()));
    }
    for seg in prefix.split('/') {
        if seg.is_empty() {
            return Err(SetKeyError::EmptyPrefixSegment(prefix.to_string()));
        }
        if seg == "." || seg == ".." {
            return Err(SetKeyError::DotSegmentInPrefix(prefix.to_string()));
        }
        if !seg.bytes().all(is_valid_char) {
            return Err(SetKeyError::InvalidPrefixCharacter(prefix.to_string()));
        }
    }
    Ok(())
}

fn validate_set_id(set_id: &str) -> Result<(), SetKeyError> {
    if set_id.is_empty() {
        return Err(SetKeyError::EmptySetId);
    }
    if set_id.len() > 128 {
        return Err(SetKeyError::SetIdTooLong(set_id.len()));
    }
    if set_id.contains('/') {
        return Err(SetKeyError::SlashInSetId(set_id.to_string()));
    }
    if !set_id.bytes().all(is_valid_char) {
        return Err(SetKeyError::InvalidSetIdCharacter(set_id.to_string()));
    }
    Ok(())
}

fn validate_artifact_name(name: &str) -> Result<(), SetKeyError> {
    if name.is_empty() {
        return Err(SetKeyError::EmptyArtifactName);
    }
    if name.len() > 256 {
        return Err(SetKeyError::ArtifactNameTooLong(name.len()));
    }
    if name.starts_with('/') {
        return Err(SetKeyError::LeadingSlashInArtifactName(name.to_string()));
    }
    if name.ends_with('/') {
        return Err(SetKeyError::TrailingSlashInArtifactName(name.to_string()));
    }
    for seg in name.split('/') {
        if seg.is_empty() {
            return Err(SetKeyError::EmptyArtifactNameSegment(name.to_string()));
        }
        if seg == "." || seg == ".." {
            return Err(SetKeyError::DotSegmentInArtifactName(name.to_string()));
        }
        if !seg.bytes().all(is_valid_char) {
            return Err(SetKeyError::InvalidArtifactNameCharacter(name.to_string()));
        }
    }
    Ok(())
}

/// Key layout mapping set ids to immutable object storage keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetKeyLayout {
    prefix: String,
}

impl SetKeyLayout {
    /// Construct a layout under `prefix`.
    ///
    /// An empty prefix places keys at top-level. Non-empty prefixes must be
    /// `/`-separated segments of `[A-Za-z0-9._-]` without leading/trailing `/`
    /// or empty/dot segments.
    pub fn new(prefix: &str) -> Result<Self, SetKeyError> {
        validate_prefix(prefix)?;
        Ok(Self {
            prefix: prefix.to_string(),
        })
    }

    /// Return the configured prefix.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Key for an immutable artifact within a set: `<prefix>/sets/<set_id>/artifacts/<name>`.
    pub fn artifact_key(&self, set_id: &str, name: &str) -> Result<String, SetKeyError> {
        validate_set_id(set_id)?;
        validate_artifact_name(name)?;
        if self.prefix.is_empty() {
            Ok(format!("sets/{set_id}/artifacts/{name}"))
        } else {
            Ok(format!("{}/sets/{set_id}/artifacts/{name}", self.prefix))
        }
    }

    /// Key for the manifest marking set completion: `<prefix>/sets/<set_id>/manifest.json`.
    pub fn manifest_key(&self, set_id: &str) -> Result<String, SetKeyError> {
        validate_set_id(set_id)?;
        if self.prefix.is_empty() {
            Ok(format!("sets/{set_id}/manifest.json"))
        } else {
            Ok(format!("{}/sets/{set_id}/manifest.json", self.prefix))
        }
    }

    /// Key for the latest successful backup pointer: `<prefix>/latest.json`.
    pub fn latest_key(&self) -> String {
        if self.prefix.is_empty() {
            "latest.json".to_string()
        } else {
            format!("{}/latest.json", self.prefix)
        }
    }

    /// Common prefix for all objects in a set: `<prefix>/sets/<set_id>/`.
    pub fn set_prefix(&self, set_id: &str) -> Result<String, SetKeyError> {
        validate_set_id(set_id)?;
        if self.prefix.is_empty() {
            Ok(format!("sets/{set_id}/"))
        } else {
            Ok(format!("{}/sets/{set_id}/", self.prefix))
        }
    }

    /// Parse the set id from any key that lies under `<prefix>/sets/<id>/`.
    ///
    /// Returns `None` if the key does not match this layout's prefix or does
    /// not lie directly under a valid set prefix.
    pub fn parse_set_id<'a>(&self, key: &'a str) -> Option<&'a str> {
        let rem = if self.prefix.is_empty() {
            key.strip_prefix("sets/")?
        } else {
            key.strip_prefix(&self.prefix)?.strip_prefix("/sets/")?
        };
        let (id, _rest) = rem.split_once('/')?;
        if id.is_empty() || id.len() > 128 || !id.bytes().all(is_valid_char) {
            return None;
        }
        Some(id)
    }
}
