use std::path::Path;

use storage_durable::FsyncPolicy;

use crate::{LocalObjectStore, Object, ObjectMeta, ObjectStore, PutCondition, Result};

/// Non-durable local object store for rebuildable scratch data.
///
/// It is not durable: it never calls fsync, so an OS crash can lose or tear
/// objects. Use it only for data that can be rebuilt. For durable data, use
/// `LocalObjectStore`.
pub struct ScratchObjectStore {
    inner: LocalObjectStore,
}

impl ScratchObjectStore {
    /// Opens a non-durable store at `root`. Writes use `FsyncPolicy::Os`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inner: LocalObjectStore::open_with_fsync(root, FsyncPolicy::Os)?,
        })
    }

    /// The flush policy of this store. Always `FsyncPolicy::Os`.
    pub fn fsync_policy(&self) -> FsyncPolicy {
        self.inner.fsync_policy()
    }
}

impl ObjectStore for ScratchObjectStore {
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
        content_type: &str,
        condition: PutCondition,
    ) -> Result<ObjectMeta> {
        self.inner.put(key, bytes, content_type, condition)
    }

    fn get(&self, key: &str) -> Result<Object> {
        self.inner.get(key)
    }

    fn head(&self, key: &str) -> Result<ObjectMeta> {
        self.inner.head(key)
    }

    fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        self.inner.list(prefix)
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.inner.delete(key)
    }
}
