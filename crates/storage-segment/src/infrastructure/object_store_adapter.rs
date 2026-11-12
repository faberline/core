use std::sync::Arc;

use storage_object::{ObjectStore, ObjectStoreError, ObjectVersion, PutCondition};

use super::put_immutable::put_immutable;
use crate::domain::{ArchiveObject, ArchivedObject, ImmutableObjectStore, Result, SegmentError};

/// The write-once object store port over a storage-object `ObjectStore`.
#[derive(Clone)]
pub(crate) struct ObjectStoreAdapter {
    store: Arc<dyn ObjectStore>,
}

impl ObjectStoreAdapter {
    pub(crate) fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }
}

impl ImmutableObjectStore for ObjectStoreAdapter {
    fn put_page(&self, key: &str, bytes: &[u8]) -> Result<()> {
        match self
            .store
            .put(key, bytes, "application/json", PutCondition::IfAbsent)
        {
            Ok(_) => Ok(()),
            Err(ObjectStoreError::PreconditionFailed { .. }) => {
                let existing = self.store.get(key)?;
                if existing.bytes() != bytes {
                    return Err(SegmentError::ImmutableObjectChanged {
                        key: key.to_string(),
                    });
                }
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn get(&self, key: &str) -> Result<Vec<u8>> {
        Ok(self.store.get(key)?.into_parts().1)
    }

    fn delete(&self, key: &str) -> Result<()> {
        Ok(self.store.delete(key)?)
    }

    fn put_object(&self, object: ArchiveObject) -> Result<ArchivedObject> {
        put_immutable(self.store.as_ref(), object)
    }

    fn create_marker(&self, key: &str, bytes: &[u8]) -> Result<bool> {
        match self
            .store
            .put(key, bytes, "application/json", PutCondition::IfAbsent)
        {
            Ok(_) => Ok(true),
            Err(ObjectStoreError::PreconditionFailed { .. }) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn read_marker(&self, key: &str) -> Result<Option<(Vec<u8>, String)>> {
        match self.store.get(key) {
            Ok(object) => {
                let version = object.meta().version().as_str().to_string();
                Ok(Some((object.into_parts().1, version)))
            }
            Err(ObjectStoreError::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn replace_marker(&self, key: &str, bytes: &[u8], expected_version: &str) -> Result<()> {
        let condition = PutCondition::IfVersion(ObjectVersion::new(expected_version));
        match self.store.put(key, bytes, "application/json", condition) {
            Ok(_) => Ok(()),
            Err(ObjectStoreError::PreconditionFailed { .. }) => {
                Err(SegmentError::StagingConflict {
                    key: key.to_string(),
                })
            }
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests;
