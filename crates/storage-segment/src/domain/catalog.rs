//! Catalog values: page limits and format versions, entries, pages and
//! page references, roots and write results, the key and page-bound rules,
//! and the pure page codec and content hash.

mod bounds;
mod content_hash;
mod entry;
mod key;
mod limits;
mod mutation;
mod page;
mod page_codec;
mod root;

pub(crate) use bounds::{child_index, page_bounds};
pub(crate) use content_hash::hex_sha256;
pub use entry::CatalogEntry;
pub(crate) use key::{lexicographic_successor, validate_catalog_key};
pub(crate) use limits::{CATALOG_FORMAT_VERSION, CATALOG_PAGE_FORMAT_VERSION};
pub use limits::{DEFAULT_CATALOG_PAGE_BYTES, MAX_ABORT_TRACKED_CATALOG_PAGES};
pub use mutation::{CatalogMutation, StreamingCatalogAbort, StreamingCatalogBuild};
pub use page::CatalogPageRef;
pub(crate) use page::{validate_page_body, CatalogPage, CatalogPageBody};
pub(crate) use page_codec::encode_page;
pub use root::CatalogRoot;
