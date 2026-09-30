use super::{ArchiveObject, ArchivedObject, Result};

/// Write-once object storage for catalog pages and archive objects. The
/// catalog and the archive coordinator write through it; infrastructure
/// implements it over a storage-object `ObjectStore`.
pub(crate) trait ImmutableObjectStore: Send + Sync {
    /// Write a catalog page under `key` only if the key is absent. An
    /// existing page with identical bytes is accepted; other bytes are
    /// `ImmutableObjectChanged`.
    fn put_page(&self, key: &str, bytes: &[u8]) -> Result<()>;

    /// Read the bytes stored under `key`.
    fn get(&self, key: &str) -> Result<Vec<u8>>;

    /// Delete the object stored under `key`.
    fn delete(&self, key: &str) -> Result<()>;

    /// Write an archive object only if its key is absent. An existing object
    /// is accepted only when its bytes and content type are identical.
    fn put_object(&self, object: ArchiveObject) -> Result<ArchivedObject>;
}
