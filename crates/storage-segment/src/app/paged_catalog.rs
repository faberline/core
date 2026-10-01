use std::sync::Arc;

use storage_object::ObjectStore;

use crate::application::PagedCatalog;
use crate::domain::{Result, DEFAULT_CATALOG_PAGE_BYTES};
use crate::infrastructure::ObjectStoreAdapter;

impl PagedCatalog {
    pub fn new(store: Arc<dyn ObjectStore>, prefix: impl Into<String>) -> Result<Self> {
        Self::with_page_bytes(store, prefix, DEFAULT_CATALOG_PAGE_BYTES)
    }

    pub fn with_page_bytes(
        store: Arc<dyn ObjectStore>,
        prefix: impl Into<String>,
        page_bytes_limit: usize,
    ) -> Result<Self> {
        Self::from_port(
            Arc::new(ObjectStoreAdapter::new(store)),
            prefix,
            page_bytes_limit,
        )
    }
}
