//! Where backups go and how they are kept: the destination and policy
//! value types that CRDs embed.

mod destination;
mod destination_error;
mod policy;
mod policy_error;

pub use destination::{BackupDestination, SchemeInfo, SUPPORTED_SCHEMES};
pub use destination_error::DestinationError;
pub use policy::{BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};
pub use policy_error::PolicyError;
