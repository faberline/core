use std::{collections::BTreeSet, sync::Arc};

use crate::domain::{
    ArchiveCommit, ArchiveObject, ArchivedObject, ImmutableObjectStore, Result, SegmentError,
};

/// Starts manifest-last archive transactions. The public constructor that
/// takes a storage-object `ObjectStore` lives in the composition root.
#[derive(Clone)]
pub struct ArchiveCoordinator {
    objects: Arc<dyn ImmutableObjectStore>,
}

impl ArchiveCoordinator {
    pub(crate) fn from_port(objects: Arc<dyn ImmutableObjectStore>) -> Self {
        Self { objects }
    }

    pub fn begin(&self) -> ArchiveTransaction {
        ArchiveTransaction {
            objects: self.objects.clone(),
            written: Vec::new(),
            keys: BTreeSet::new(),
            failed: false,
        }
    }
}

pub struct ArchiveTransaction {
    objects: Arc<dyn ImmutableObjectStore>,
    written: Vec<ArchivedObject>,
    keys: BTreeSet<String>,
    failed: bool,
}

impl ArchiveTransaction {
    /// Write one immutable object. A retry accepts an existing byte-identical
    /// object, but it rejects content changes under the same key.
    pub fn put(&mut self, object: ArchiveObject) -> Result<ArchivedObject> {
        if self.failed {
            return Err(SegmentError::TransactionFailed);
        }
        if !self.keys.insert(object.key.clone()) {
            return Err(SegmentError::DuplicateObject { key: object.key });
        }
        match self.objects.put_object(object) {
            Ok(receipt) => {
                self.written.push(receipt.clone());
                Ok(receipt)
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    /// Write the manifest last. No `ArchiveCommit` exists before this call.
    pub fn commit(mut self, manifest: ArchiveObject) -> Result<ArchiveCommit> {
        if self.failed {
            return Err(SegmentError::TransactionFailed);
        }
        if self.keys.contains(&manifest.key) {
            return Err(SegmentError::ManifestKeyCollision { key: manifest.key });
        }
        let manifest = match self.objects.put_object(manifest) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        Ok(ArchiveCommit {
            objects: self.written,
            manifest,
        })
    }
}
