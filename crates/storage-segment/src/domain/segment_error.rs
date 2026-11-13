use thiserror::Error;

use super::StagingState;

#[derive(Debug, Error)]
pub enum SegmentError {
    #[error("segment codec failed: {message}")]
    Codec { message: String },
    #[error("segment partition is invalid: {partition}")]
    InvalidPartition { partition: String },
    #[error("archive object key is repeated: {key}")]
    DuplicateObject { key: String },
    #[error("archive manifest key was already used by an immutable object: {key}")]
    ManifestKeyCollision { key: String },
    #[error("archive transaction cannot commit after an earlier upload failed")]
    TransactionFailed,
    #[error("existing immutable object {key} has different content")]
    ImmutableObjectChanged { key: String },
    #[error("catalog key is invalid: {key}")]
    InvalidCatalogKey { key: String },
    #[error("catalog contains a duplicate key: {key}")]
    DuplicateCatalogKey { key: String },
    #[error("streaming catalog keys are not strictly increasing: {key}")]
    UnsortedCatalogKey { key: String },
    #[error("catalog page exceeds {limit} bytes")]
    CatalogPageTooLarge { limit: usize },
    #[error("catalog data is corrupt: {message}")]
    CorruptCatalog { message: String },
    #[error("catalog serialization failed: {message}")]
    Serialization { message: String },
    #[error("archive intent is corrupt: {message}")]
    CorruptArchiveIntent { message: String },
    #[error("archive intent format version {version} is not supported")]
    UnsupportedArchiveIntentVersion { version: u64 },
    #[error("staging transition {from:?} to {to:?} is not allowed")]
    IllegalStagingTransition {
        from: StagingState,
        to: StagingState,
    },
    #[error("no staging marker exists for segment {segment_key}")]
    StagingMarkerMissing { segment_key: String },
    #[error("staging marker {key} was changed by another writer")]
    StagingConflict { key: String },
    #[error("staging marker is corrupt: {message}")]
    CorruptStagingMarker { message: String },
    #[error("staging marker format version {version} is not supported")]
    UnsupportedStagingMarkerVersion { version: u64 },
    /// The object store failed. Infrastructure wraps storage-object's
    /// `ObjectStoreError` here, and the message is that error's own.
    #[error(transparent)]
    ObjectStore(Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T> = std::result::Result<T, SegmentError>;
