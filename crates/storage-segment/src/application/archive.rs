use std::{collections::BTreeSet, sync::Arc};

use crate::domain::{
    staging_object_key, ArchiveCommit, ArchiveIntent, ArchiveObject, ArchivedObject, GcReport,
    ImmutableObjectStore, Result, SegmentError, StagingMarker, StagingState, ARCHIVE_INTENT_PREFIX,
    STAGING_MARKER_PREFIX,
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
    /// state. A second call after a crash never writes a second marker.
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

    /// The stored staging marker of a segment, or `None`.
    pub fn staging_marker(&self, segment_key: &str) -> Result<Option<StagingMarker>> {
        match self.objects.read_marker(&staging_object_key(segment_key))? {
            Some((bytes, _)) => Ok(Some(StagingMarker::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Move the marker of a segment to `next` when the transition is legal.
    /// An illegal pair is refused before any write.
    pub fn advance_staging(&self, segment_key: &str, next: StagingState) -> Result<StagingMarker> {
        self.advance(segment_key, next, None)
    }

    /// Like `advance_staging`; records `at` (caller seconds) when `next` is `Committed` or `GcPending`.
    pub fn advance_staging_at(
        &self,
        segment_key: &str,
        next: StagingState,
        at: u64,
    ) -> Result<StagingMarker> {
        self.advance(segment_key, next, Some(at))
    }

    fn advance(
        &self,
        segment_key: &str,
        next: StagingState,
        at: Option<u64>,
    ) -> Result<StagingMarker> {
        let key = staging_object_key(segment_key);
        let Some((bytes, version)) = self.objects.read_marker(&key)? else {
            return Err(SegmentError::StagingMarkerMissing {
                segment_key: segment_key.to_string(),
            });
        };
        let current = StagingMarker::decode(&bytes)?;
        let moved = match at {
            Some(at) => current.move_to_at(next, at)?,
            None => current.move_to(next)?,
        };
        self.objects
            .replace_marker(&key, &moved.encode()?, &version)?;
        Ok(moved)
    }

    /// Delete a committed segment and its marker once the retention window has passed. `now` and `retention` are caller seconds.
    pub fn delete_committed(&self, segment_key: &str, now: u64, retention: u64) -> Result<()> {
        let Some(marker) = self.staging_marker(segment_key)? else {
            return Err(SegmentError::StagingMarkerMissing {
                segment_key: segment_key.to_string(),
            });
        };
        if marker.state != StagingState::Committed {
            return Err(SegmentError::SegmentNotCommitted {
                segment_key: segment_key.to_string(),
            });
        }
        let until = marker
            .committed_at
            .unwrap_or(u64::MAX)
            .saturating_add(retention);
        if now < until {
            return Err(SegmentError::InsideRetentionWindow {
                segment_key: segment_key.to_string(),
                until,
            });
        }
        self.objects.delete_if_present(segment_key)?;
        self.objects
            .delete_if_present(&staging_object_key(segment_key))?;
        Ok(())
    }

    /// Delete every `GcPending` object whose `gc_pending_at + retention` is not later than `now`. Committed, intent, uploaded and unstaged objects are never touched.
    pub fn collect_garbage(&self, now: u64, retention: u64) -> Result<GcReport> {
        let mut deleted = Vec::new();
        for key in self.objects.list_keys(STAGING_MARKER_PREFIX)? {
            let Some((bytes, _)) = self.objects.read_marker(&key)? else {
                continue;
            };
            let marker = StagingMarker::decode(&bytes)?;
            if marker.state != StagingState::GcPending {
                continue;
            }
            let Some(gc_pending_at) = marker.gc_pending_at else {
                continue;
            };
            if now < gc_pending_at.saturating_add(retention) {
                continue;
            }
            // Segment first, marker second: a crash between the two is
            // repaired by the next pass, which finds the segment gone.
            let existed = self.objects.delete_if_present(&marker.segment_key)?;
            self.objects.delete_if_present(&key)?;
            if existed {
                deleted.push(marker.segment_key);
            }
        }
        Ok(GcReport { deleted })
    }

    /// Write the intent record before the upload starts.
    pub fn write_intent(&self, intent: &ArchiveIntent) -> Result<ArchivedObject> {
        self.objects.put_object(ArchiveObject::new(
            intent.object_key(),
            intent.encode()?,
            "application/json",
        ))
    }

    /// Every intent record under the reserved prefix. This issue never removes
    /// an intent; the staging issue (#267) decides when one stops being pending.
    pub fn pending_intents(&self) -> Result<Vec<ArchiveIntent>> {
        let mut intents = Vec::new();
        for key in self.objects.list_keys(ARCHIVE_INTENT_PREFIX)? {
            intents.push(ArchiveIntent::decode(&self.objects.get(&key)?)?);
        }
        Ok(intents)
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
