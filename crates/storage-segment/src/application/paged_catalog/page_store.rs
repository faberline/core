use storage_object::{ObjectStoreError, PutCondition};

use super::PagedCatalog;
use crate::domain::{
    page_bounds, validate_page_body, CatalogPage, CatalogPageBody, CatalogPageRef, Result,
    SegmentError, CATALOG_PAGE_FORMAT_VERSION,
};
use crate::infrastructure::{encode_page, hex_sha256};

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
        match self
            .store
            .put(&key, &bytes, "application/json", PutCondition::IfAbsent)
        {
            Ok(_) => {}
            Err(ObjectStoreError::PreconditionFailed { .. }) => {
                let existing = self.store.get(&key)?;
                if existing.bytes != bytes {
                    return Err(SegmentError::ImmutableObjectChanged { key });
                }
            }
            Err(error) => return Err(error.into()),
        }
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
        let object = self.store.get(&reference.key)?;
        if object.bytes.len() as u64 != reference.bytes
            || hex_sha256(&object.bytes) != reference.sha256
            || object.bytes.len() > self.page_bytes_limit
        {
            return Err(SegmentError::CorruptCatalog {
                message: format!("page {} failed size or hash validation", reference.key),
            });
        }
        let page: CatalogPage =
            serde_json::from_slice(&object.bytes).map_err(|error| SegmentError::Serialization {
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
