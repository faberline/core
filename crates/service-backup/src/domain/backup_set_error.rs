//! Errors returned when validating, serializing, or verifying backup sets.

/// Errors returned by [`BackupSetV1`](crate::BackupSetV1) validation,
/// deserialization, or [`ArtifactVerifier`](crate::ArtifactVerifier).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BackupSetError {
    /// The backup set manifest version is unsupported (only version 1 is supported).
    #[error("unsupported backup set version `{0}`; expected version 1")]
    UnsupportedVersion(u32),

    /// The set id contains invalid characters or is empty.
    #[error("backup set id `{0}` is invalid; must be non-empty and match [A-Za-z0-9._-]")]
    InvalidSetId(String),

    /// The producer contains invalid characters or is empty.
    #[error("backup producer `{0}` is invalid; must be non-empty and match [A-Za-z0-9._-]")]
    InvalidProducer(String),

    /// The artifact list is empty.
    #[error("backup set must contain at least one artifact")]
    EmptyArtifacts,

    /// The artifact count exceeds the maximum allowed limit.
    #[error("backup set artifact count {count} exceeds maximum of {max}")]
    TooManyArtifacts {
        /// Actual number of artifacts.
        count: usize,
        /// Maximum allowed number of artifacts.
        max: usize,
    },

    /// The artifact name is invalid.
    #[error("artifact name `{0}` is invalid")]
    InvalidArtifactName(String),

    /// An artifact name appears more than once in the manifest.
    #[error("duplicate artifact name `{0}`")]
    DuplicateArtifactName(String),

    /// The artifact digest does not match the expected format.
    #[error("invalid digest `{0}`; expected 'sha256:' followed by 64 lowercase hex characters")]
    InvalidDigest(String),

    /// The artifact content type is empty.
    #[error("artifact content type must not be empty")]
    EmptyContentType,

    /// The service metadata is present but is not a JSON object.
    #[error("service metadata must be a JSON object")]
    InvalidServiceMetadata,

    /// The manifest byte length exceeds the maximum allowed limit.
    #[error("manifest size {size} exceeds maximum of {max} bytes")]
    ManifestTooLarge {
        /// Size of the manifest in bytes.
        size: usize,
        /// Maximum allowed manifest size in bytes.
        max: usize,
    },

    /// Incoming artifact bytes exceeded the declared size.
    #[error("artifact bytes {actual} exceeded declared size {expected}")]
    BytesExceeded {
        /// Declared size in bytes.
        expected: u64,
        /// Actual bytes seen so far.
        actual: u64,
    },

    /// The artifact finished with fewer bytes than declared.
    #[error("artifact size {actual} does not match declared size {expected}")]
    ShortSize {
        /// Declared size in bytes.
        expected: u64,
        /// Actual bytes observed.
        actual: u64,
    },

    /// The computed artifact digest does not match the declared digest.
    #[error("artifact digest mismatch: expected `{expected}`, computed `{actual}`")]
    DigestMismatch {
        /// Declared digest string.
        expected: String,
        /// Computed digest string.
        actual: String,
    },

    /// Failed to parse or serialize JSON.
    #[error("json error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for BackupSetError {
    fn from(err: serde_json::Error) -> Self {
        BackupSetError::Json(err.to_string())
    }
}
