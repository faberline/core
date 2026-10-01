use super::PagedCatalog;
use crate::domain::{
    encode_page, hex_sha256, page_bounds, validate_page_body, CatalogPage, CatalogPageBody,
    CatalogPageRef, Result, SegmentError, CATALOG_PAGE_FORMAT_VERSION,
};

impl PagedCatalog {
    pub(super) fn store_page(&self, body: CatalogPageBody) -> Result<CatalogPageRef> {
        validate_page_body(&body)?;
        let bytes = encode_page(&body)?;
        if bytes.len() > self.page_bytes_limit {
            return Err(SegmentError::CatalogPageTooLarge {
                limit: self.page_bytes_limit,
            });
        }
        let sha256 = hex_sha256(&bytes);
        let key = format!("{}/pages/{sha256}.json", self.prefix);
        self.objects.put_page(&key, &bytes)?;
        let (entry_count, first_key, last_key) = page_bounds(&body)?;
        Ok(CatalogPageRef {
            key,
            sha256,
            bytes: bytes.len() as u64,
            entry_count,
            first_key,
            last_key,
        })
    }

    pub(super) fn load_page(&self, reference: &CatalogPageRef) -> Result<CatalogPageBody> {
        let bytes = self.objects.get(&reference.key)?;
        if bytes.len() as u64 != reference.bytes
            || hex_sha256(&bytes) != reference.sha256
            || bytes.len() > self.page_bytes_limit
        {
            return Err(SegmentError::CorruptCatalog {
                message: format!("page {} failed size or hash validation", reference.key),
            });
        }
        let page: CatalogPage =
            serde_json::from_slice(&bytes).map_err(|error| SegmentError::Serialization {
                message: error.to_string(),
            })?;
        if page.format_version != CATALOG_PAGE_FORMAT_VERSION {
            return Err(SegmentError::CorruptCatalog {
                message: format!("page {} has an unsupported format", reference.key),
            });
        }
        validate_page_body(&page.body)?;
        let (count, first, last) = page_bounds(&page.body)?;
        if count != reference.entry_count
            || first != reference.first_key
            || last != reference.last_key
        {
            return Err(SegmentError::CorruptCatalog {
                message: format!("page {} disagrees with its reference", reference.key),
            });
        }
        Ok(page.body)
    }
}
