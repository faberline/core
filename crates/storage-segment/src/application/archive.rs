use std::{collections::BTreeSet, sync::Arc};

use crate::domain::{
    staging_object_key, ArchiveCommit, ArchiveObject, ArchivedObject, ImmutableObjectStore, Result,
    SegmentError, StagingMarker, StagingState,
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

    /// Start staging one segment in the `Intent` state, or resume the stored
    /// state. A second call never writes a second marker.
    pub fn begin_staging(&self, segment_key: &str) -> Result<StagingMarker> {
        let marker = StagingMarker::new(segment_key);
        if self
            .objects
            .create_marker(&marker.object_key(), &marker.encode()?)?
        {
            return Ok(marker);
        }
        self.staging_marker(segment_key)?
            .ok_or_else(|| SegmentError::StagingMarkerMissing {
                segment_key: segment_key.to_string(),
            })
    }

    /// The stored staging marker, or `None`.
    pub fn staging_marker(&self, segment_key: &str) -> Result<Option<StagingMarker>> {
        match self.objects.read_marker(&staging_object_key(segment_key))? {
            Some((bytes, _)) => Ok(Some(StagingMarker::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Move the marker to `next` when the transition is legal. An illegal
    /// transition is refused before any write.
    pub fn advance_staging(&self, segment_key: &str, next: StagingState) -> Result<StagingMarker> {
        let key = staging_object_key(segment_key);
        let Some((bytes, version)) = self.objects.read_marker(&key)? else {
            return Err(SegmentError::StagingMarkerMissing {
                segment_key: segment_key.to_string(),
            });
        };
        let moved = StagingMarker::decode(&bytes)?.move_to(next)?;
        self.objects
            .replace_marker(&key, &moved.encode()?, &version)?;
        Ok(moved)
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
