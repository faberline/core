//! Use cases over an object store: the copy-on-write paged catalog and its
//! readers, and the manifest-last archive coordinator and transaction.

mod archive;
mod paged_catalog;

pub use archive::{ArchiveCoordinator, ArchiveTransaction};
pub use paged_catalog::{CatalogPageKeyReader, CatalogReader, PagedCatalog};
