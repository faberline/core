use std::{collections::BTreeSet, sync::Arc};

use storage_object::ObjectStore;

use crate::domain::{ArchiveCommit, ArchiveObject, ArchivedObject, Result, SegmentError};
use crate::infrastructure::put_immutable;

#[derive(Clone)]
pub struct ArchiveCoordinator {
    store: Arc<dyn ObjectStore>,
}

impl ArchiveCoordinator {
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    pub fn begin(&self) -> ArchiveTransaction {
        ArchiveTransaction {
            store: self.store.clone(),
            objects: Vec::new(),
            keys: BTreeSet::new(),
            failed: false,
        }
    }
}

pub struct ArchiveTransaction {
    store: Arc<dyn ObjectStore>,
    objects: Vec<ArchivedObject>,
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
        match put_immutable(self.store.as_ref(), object) {
            Ok(receipt) => {
                self.objects.push(receipt.clone());
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
        let manifest = match put_immutable(self.store.as_ref(), manifest) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        Ok(ArchiveCommit {
            objects: self.objects,
            manifest,
        })
    }
}
