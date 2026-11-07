//! Where backups go and how they are kept: the destination and policy
//! value types that CRDs embed, the ports a backup run reads a snapshot
//! from and writes it through, and the bearer token source the admin
//! snapshot transport authenticates with.

mod artifact_verifier;
mod backup_set;
mod backup_set_error;
mod backup_sink;
mod backup_sink_error;
#[cfg(feature = "http-client")]
mod bearer_token_error;
#[cfg(feature = "http-client")]
mod bearer_token_source;
mod destination;
mod destination_error;
mod policy;
mod policy_error;
#[cfg(feature = "http-client")]
mod snapshot_source;
#[cfg(feature = "http-client")]
mod snapshot_source_error;

pub use artifact_verifier::ArtifactVerifier;
pub use backup_set::{
    BackupArtifact, BackupSetV1, MAX_ARTIFACTS, MAX_ARTIFACT_NAME_BYTES, MAX_MANIFEST_BYTES,
};
pub use backup_set_error::BackupSetError;
pub use backup_sink::BackupSink;
pub use backup_sink_error::BackupSinkError;
#[cfg(feature = "http-client")]
pub use bearer_token_error::BearerTokenError;
#[cfg(feature = "http-client")]
pub use bearer_token_source::BearerTokenSource;
pub use destination::{BackupDestination, SchemeInfo, SUPPORTED_SCHEMES};
pub use destination_error::DestinationError;
pub use policy::{BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};
pub use policy_error::PolicyError;
#[cfg(feature = "http-client")]
pub use snapshot_source::SnapshotSource;
#[cfg(feature = "http-client")]
pub use snapshot_source_error::SnapshotSourceError;
