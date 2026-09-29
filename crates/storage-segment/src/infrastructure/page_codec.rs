use crate::domain::{
    CatalogPage, CatalogPageBody, Result, SegmentError, CATALOG_PAGE_FORMAT_VERSION,
};

pub(crate) fn encode_page(body: &CatalogPageBody) -> Result<Vec<u8>> {
    serde_json::to_vec(&CatalogPage {
        format_version: CATALOG_PAGE_FORMAT_VERSION,
        body: body.clone(),
    })
    .map_err(|error| SegmentError::Serialization {
        message: error.to_string(),
    })
}
