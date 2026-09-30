use super::limits::CATALOG_PAGE_FORMAT_VERSION;
use super::page::{CatalogPage, CatalogPageBody};
use crate::domain::{Result, SegmentError};

/// The stored JSON bytes of a catalog page at the current page format.
pub(crate) fn encode_page(body: &CatalogPageBody) -> Result<Vec<u8>> {
    serde_json::to_vec(&CatalogPage {
        format_version: CATALOG_PAGE_FORMAT_VERSION,
        body: body.clone(),
    })
    .map_err(|error| SegmentError::Serialization {
        message: error.to_string(),
    })
}
