//! The composition root: wiring that may use every layer.
//!
//! The public `PagedCatalog` and `ArchiveCoordinator` constructors take a
//! storage-object `ObjectStore`, wrap it in the infrastructure adapter and
//! hand the application the write-once object store port.

mod archive;
mod paged_catalog;
