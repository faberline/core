//! Domain error reporting why a restore ledger operation was rejected.

use super::{ArtifactStage, RestorePhase};

/// Errors returned by [`RestoreLedger`](crate::RestoreLedger) operations.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RestoreLedgerError {
    /// The ledger JSON or manifest version is unsupported (only version 1 is supported).
    #[error("unsupported restore ledger version `{0}`; expected version 1")]
    UnsupportedVersion(u32),

    /// The JSON byte length exceeds the maximum allowed limit of 1 MiB.
    #[error("restore ledger JSON size {size} exceeds maximum of {max} bytes")]
    OversizedJson {
        /// Actual size in bytes.
        size: usize,
        /// Maximum allowed size in bytes.
        max: usize,
    },

    /// The set id contains invalid characters, is empty, or exceeds 128 bytes.
    #[error("restore set id `{0}` is invalid; must be non-empty, at most 128 bytes, and match [A-Za-z0-9._-]")]
    InvalidSetId(String),

    /// The manifest digest is invalid (expected 'sha256:' followed by 64 lowercase hex characters).
    #[error("restore manifest digest `{0}` is invalid; must be 'sha256:' followed by 64 lowercase hex characters")]
    InvalidManifestDigest(String),

    /// The artifact list is empty.
    #[error("restore ledger must contain at least one artifact")]
    EmptyArtifacts,

    /// The artifact count exceeds the maximum allowed limit of 1024.
    #[error("restore ledger artifact count {count} exceeds maximum of {max}")]
    TooManyArtifacts {
        /// Actual count of artifacts.
        count: usize,
        /// Maximum allowed count of artifacts.
        max: usize,
    },

    /// An artifact name is empty.
    #[error("artifact name must not be empty")]
    EmptyArtifactName,

    /// An artifact name appears more than once.
    #[error("duplicate artifact name `{0}`")]
    DuplicateArtifact(String),

    /// An artifact was not found in the manifest artifact list.
    #[error("unknown artifact `{0}`")]
    UnknownArtifact(String),

    /// An illegal artifact transition was attempted (e.g. marking applied on an unverified artifact).
    #[error("illegal transition: cannot mark applied on artifact `{0}` while in stage `{1:?}`")]
    IllegalArtifactTransition(String, ArtifactStage),

    /// A transition from the wrong phase was attempted.
    #[error("illegal phase transition: cannot transition from phase `{current:?}` to `{target:?}`: {reason}")]
    IllegalPhaseTransition {
        /// Current phase.
        current: RestorePhase,
        /// Target phase.
        target: RestorePhase,
        /// Description of the violation.
        reason: String,
    },

    /// Cannot transition to `Applying` while some artifacts are not verified.
    #[error("cannot begin apply while artifacts remain unverified: {0:?}")]
    UnverifiedArtifacts(Vec<String>),

    /// Cannot commit while some artifacts are not applied.
    #[error("cannot commit restore while artifacts remain unapplied: {0:?}")]
    UnappliedArtifacts(Vec<String>),

    /// Resuming against an active or committed ledger with a different set ID or different manifest digest.
    #[error("conflicting restore set: existing `{existing}`, attempted `{attempted}`")]
    ConflictingSet {
        /// The existing set ID or manifest digest.
        existing: String,
        /// The attempted set ID or manifest digest.
        attempted: String,
    },

    /// The ledger phase invariants are inconsistent with artifact stages.
    #[error("inconsistent ledger phase {phase:?}: {details}")]
    InconsistentPhase {
        /// The phase of the ledger.
        phase: RestorePhase,
        /// Description of the inconsistency.
        details: String,
    },

    /// JSON serialization or deserialization failure.
    #[error("json error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for RestoreLedgerError {
    fn from(err: serde_json::Error) -> Self {
        RestoreLedgerError::Json(err.to_string())
    }
}
