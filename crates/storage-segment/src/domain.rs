//! Segment and catalog values: the segment error, the product ports for
//! segment records, catalog pages, roots and write results, archive objects
//! and receipts, and the write-once object store port.

mod archive;
mod catalog;
mod immutable_object_store;
mod segment;
mod segment_error;

pub use archive::{ArchiveCommit, ArchiveObject, ArchivedObject, ArchivedObjectVersion};
pub(crate) use catalog::{
    child_index, encode_page, hex_sha256, lexicographic_successor, page_bounds,
    validate_catalog_key, validate_page_body, CatalogPage, CatalogPageBody, CATALOG_FORMAT_VERSION,
    CATALOG_PAGE_FORMAT_VERSION,
};
pub use catalog::{
    CatalogEntry, CatalogMutation, CatalogPageRef, CatalogRoot, StreamingCatalogAbort,
    StreamingCatalogBuild, DEFAULT_CATALOG_PAGE_BYTES, MAX_ABORT_TRACKED_CATALOG_PAGES,
};
pub(crate) use immutable_object_store::ImmutableObjectStore;
pub use segment::{Partitioner, RecordCodec};
pub use segment_error::{Result, SegmentError};
