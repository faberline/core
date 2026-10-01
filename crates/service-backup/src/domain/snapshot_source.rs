//! The port a backup run reads one snapshot from.

use std::future::Future;
use std::pin::Pin;

use super::SnapshotSourceError;

/// Where one backup run's snapshot bytes come from.
///
/// Async and dyn-compatible: `fetch_snapshot` returns a boxed future, so a
/// use case can take `&dyn SnapshotSource`. The adapter owns everything the
/// fetch needs (address, credential, limits), which is why the method takes
/// no arguments.
pub trait SnapshotSource: Send + Sync {
    /// Fetch one consistent snapshot, byte for byte.
    fn fetch_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, SnapshotSourceError>> + Send + '_>>;
}
