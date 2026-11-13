//! Shared immutable-segment and archive coordination contracts.

mod app;
mod application;
mod domain;
mod infrastructure;

pub use application::{
    ArchiveCoordinator, ArchiveTransaction, CatalogPageKeyReader, CatalogReader, PagedCatalog,
};
pub use domain::{
    ArchiveCommit, ArchiveIntent, ArchiveObject, ArchivedObject, ArchivedObjectVersion,
    CatalogEntry, CatalogMutation, CatalogPageRef, CatalogRoot, Partitioner, RecordCodec, Result,
    SegmentError, StagingMarker, StagingState, StreamingCatalogAbort, StreamingCatalogBuild,
    ARCHIVE_INTENT_FORMAT_VERSION, ARCHIVE_INTENT_PREFIX, DEFAULT_CATALOG_PAGE_BYTES,
    MAX_ABORT_TRACKED_CATALOG_PAGES, STAGING_MARKER_FORMAT_VERSION, STAGING_MARKER_PREFIX,
};
