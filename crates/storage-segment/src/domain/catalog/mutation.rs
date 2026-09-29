use super::root::CatalogRoot;
use crate::domain::SegmentError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogMutation {
    pub root: CatalogRoot,
    pub written_page_keys: Vec<String>,
    pub obsolete_page_keys: Vec<String>,
}

/// Result of a bounded-memory bulk build over an already sorted input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamingCatalogBuild {
    pub root: CatalogRoot,
    pub written_page_count: u64,
    pub peak_buffer_bytes: usize,
}

/// A failed streaming build plus every page key it may have created.
///
/// The caller can clean these keys when the catalog prefix is private to the
/// failed transaction. No root was committed, so the pages are not a catalog.
#[derive(Debug)]
pub struct StreamingCatalogAbort {
    pub error: SegmentError,
    pub written_page_keys: Vec<String>,
}
