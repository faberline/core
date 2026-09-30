//! The copy-on-write paged catalog over the write-once object store port.
//! Each child module adds one group of `PagedCatalog` methods; the public
//! constructors that take a storage-object `ObjectStore` live in the
//! composition root.

mod build;
mod lookup;
mod packing;
mod page_store;
mod reader;
mod remove;
mod streaming;
mod upsert;

use std::sync::Arc;

use crate::domain::{
    CatalogRoot, ImmutableObjectStore, Result, SegmentError, CATALOG_FORMAT_VERSION,
    DEFAULT_CATALOG_PAGE_BYTES,
};

pub use reader::{CatalogPageKeyReader, CatalogReader};

#[derive(Clone)]
pub struct PagedCatalog {
    objects: Arc<dyn ImmutableObjectStore>,
    prefix: String,
    page_bytes_limit: usize,
}

impl PagedCatalog {
    /// A catalog under `prefix` that writes pages through `objects`. The
    /// prefix must be a non-empty relative key path, and the page limit
    /// 4 KiB to `DEFAULT_CATALOG_PAGE_BYTES`.
    pub(crate) fn from_port(
        objects: Arc<dyn ImmutableObjectStore>,
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
            objects,
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
