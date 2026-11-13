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

    /// Keys of the objects under `prefix`, sorted.
    fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;

    /// Write an archive object only if its key is absent. An existing object
    /// is accepted only when its bytes and content type are identical.
    fn put_object(&self, object: ArchiveObject) -> Result<ArchivedObject>;

    /// Write a marker only if the key is absent. `true` when it was written,
    /// `false` when the key already exists (the existing marker is untouched).
    fn create_marker(&self, key: &str, bytes: &[u8]) -> Result<bool>;

    /// The bytes and the object version of a marker, or `None` when the key
    /// does not exist.
    fn read_marker(&self, key: &str) -> Result<Option<(Vec<u8>, String)>>;

    /// Overwrite a marker only if its object version is still
    /// `expected_version`; otherwise `StagingConflict`.
    fn replace_marker(&self, key: &str, bytes: &[u8], expected_version: &str) -> Result<()>;
}
