use super::{ProjectionCheckpoint, ProjectionDescriptor, ProjectionError};

/// Durable storage for the latest snapshot of each projection. The registry
/// and its handles save, restore and quarantine state through it;
/// infrastructure implements it with one versioned envelope file per
/// projection.
pub(crate) trait ProjectionStateStore: Send + Sync {
    /// Create the state root. The registry calls it once, before any handle
    /// opens.
    fn prepare_root(&self) -> Result<(), ProjectionError>;

    /// Read the saved state of projection `name`, or `None` when it has none.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ProjectionError>;

    /// Decode saved bytes and check them against `descriptor`. Returns the
    /// saved checkpoint and the state bytes it covers. An error means the
    /// bytes cannot be restored.
    fn restore(
        &self,
        descriptor: &ProjectionDescriptor,
        bytes: &[u8],
    ) -> Result<(ProjectionCheckpoint, Vec<u8>), ProjectionError>;

    /// Move the saved state of `name`, whose bytes are `bytes`, aside so the
    /// projection can be rebuilt.
    fn quarantine(&self, name: &str, bytes: &[u8]) -> Result<(), ProjectionError>;

    /// Save `state` with its `checkpoint` as the state of `name`, atomically
    /// and durably.
    fn persist(
        &self,
        name: &str,
        checkpoint: &ProjectionCheckpoint,
        state: &[u8],
    ) -> Result<(), ProjectionError>;
}
