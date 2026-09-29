//! Where backups go and how they are kept: the destination and policy
//! value types that CRDs embed.

mod destination;
mod policy;

pub use destination::{BackupDestination, SchemeInfo, SUPPORTED_SCHEMES};
pub use policy::{BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};
