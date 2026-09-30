//! Where backups go and how they are kept: the destination and policy
//! value types that CRDs embed, and the ports a backup run reads a snapshot
//! from and writes it through.

mod backup_sink;
mod backup_sink_error;
mod destination;
mod destination_error;
mod policy;
mod policy_error;
#[cfg(feature = "http-client")]
mod snapshot_source;
#[cfg(feature = "http-client")]
mod snapshot_source_error;

pub use backup_sink::BackupSink;
pub use backup_sink_error::BackupSinkError;
pub use destination::{BackupDestination, SchemeInfo, SUPPORTED_SCHEMES};
pub use destination_error::DestinationError;
pub use policy::{BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};
pub use policy_error::PolicyError;
#[cfg(feature = "http-client")]
pub use snapshot_source::SnapshotSource;
#[cfg(feature = "http-client")]
pub use snapshot_source_error::SnapshotSourceError;
