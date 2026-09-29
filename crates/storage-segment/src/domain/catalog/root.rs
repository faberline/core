use serde::{Deserialize, Serialize};

use super::page::CatalogPageRef;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogRoot {
    pub format_version: u16,
    pub height: u16,
    pub entry_count: u64,
    pub page_bytes_limit: u32,
    pub root: CatalogPageRef,
}
