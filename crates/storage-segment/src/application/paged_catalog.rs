//! The copy-on-write paged catalog over an object store. Each child module
//! adds one group of `PagedCatalog` methods.

mod build;
mod lookup;
mod packing;
mod page_store;
mod reader;
mod remove;
mod streaming;
mod upsert;

use std::sync::Arc;

use storage_object::ObjectStore;

use crate::domain::{
    CatalogRoot, Result, SegmentError, CATALOG_FORMAT_VERSION, DEFAULT_CATALOG_PAGE_BYTES,
};

pub use reader::{CatalogPageKeyReader, CatalogReader};

#[derive(Clone)]
pub struct PagedCatalog {
    store: Arc<dyn ObjectStore>,
    prefix: String,
    page_bytes_limit: usize,
}

impl PagedCatalog {
    pub fn new(store: Arc<dyn ObjectStore>, prefix: impl Into<String>) -> Result<Self> {
        Self::with_page_bytes(store, prefix, DEFAULT_CATALOG_PAGE_BYTES)
    }

    pub fn with_page_bytes(
        store: Arc<dyn ObjectStore>,
        prefix: impl Into<String>,
        page_bytes_limit: usize,
    ) -> Result<Self> {
        let prefix = prefix.into().trim_matches('/').to_string();
        if prefix.is_empty()
            || prefix
                .split('/')
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
        {
            return Err(SegmentError::InvalidCatalogKey { key: prefix });
        }
        if !(4 * 1024..=DEFAULT_CATALOG_PAGE_BYTES).contains(&page_bytes_limit) {
            return Err(SegmentError::CatalogPageTooLarge {
                limit: page_bytes_limit,
            });
        }
        Ok(Self {
            store,
            prefix,
            page_bytes_limit,
        })
    }

    fn validate_root(&self, root: &CatalogRoot) -> Result<()> {
        if root.format_version != CATALOG_FORMAT_VERSION
            || root.page_bytes_limit as usize != self.page_bytes_limit
            || root.root.entry_count != root.entry_count
            || root.root.bytes == 0
            || root.root.bytes > self.page_bytes_limit as u64
            || root.root.sha256.len() != 64
        {
            return Err(SegmentError::CorruptCatalog {
                message: "root metadata is invalid".to_string(),
            });
        }
        Ok(())
    }
}
