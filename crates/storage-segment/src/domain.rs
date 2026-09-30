//! Segment and catalog values: the segment error, the product ports for
//! segment records, catalog pages, roots and write results, and archive
//! objects and receipts.

mod archive;
mod catalog;
mod segment;
mod segment_error;

pub use archive::{ArchiveCommit, ArchiveObject, ArchivedObject};
pub(crate) use catalog::{
    child_index, lexicographic_successor, page_bounds, validate_catalog_key, validate_page_body,
    CatalogPage, CatalogPageBody, CATALOG_FORMAT_VERSION, CATALOG_PAGE_FORMAT_VERSION,
};
pub use catalog::{
    CatalogEntry, CatalogMutation, CatalogPageRef, CatalogRoot, StreamingCatalogAbort,
    StreamingCatalogBuild, DEFAULT_CATALOG_PAGE_BYTES, MAX_ABORT_TRACKED_CATALOG_PAGES,
};
pub use segment::{Partitioner, RecordCodec};
pub use segment_error::{Result, SegmentError};
