//! Persisted restore progress ledger and state transitions.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::RestoreLedgerError;

/// The maximum size of a serialized restore ledger JSON document in bytes (1 MiB).
pub const MAX_RESTORE_LEDGER_BYTES: usize = 1024 * 1024;

/// The maximum number of artifacts allowed in a restore ledger.
pub const MAX_RESTORE_LEDGER_ARTIFACTS: usize = 1024;

/// The maximum byte length of a set ID.
pub const MAX_SET_ID_BYTES: usize = 128;

/// Progress stage of a single artifact within a restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactStage {
    /// Artifact download/staging has not yet completed and verified.
    Pending,
    /// Artifact content and checksum have been verified on disk.
    Verified,
    /// Service adapter has applied the artifact to the state machine.
    Applied,
}

/// Overall phase of the restore operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestorePhase {
    /// Artifacts are being downloaded, staged, and verified.
    Staging,
    /// All artifacts are verified; service apply is in progress.
    Applying,
    /// All artifacts have been applied and the restore is committed.
    Committed,
}

/// Outcome of attempting to resume a restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumePoint {
    /// No existing ledger was present; a fresh restore was initialized.
    Fresh,
    /// Existing ledger was found and is in progress; restore should continue.
    Continue,
    /// Existing restore is already committed; caller can proceed without re-applying.
    AlreadyCommitted,
}

/// A single artifact record in the restore ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactEntry {
    /// Name of the artifact as declared in the manifest.
    pub name: String,
    /// Declared size of the artifact in bytes.
    pub size_bytes: u64,
    /// Current stage of the artifact.
    pub stage: ArtifactStage,
}

/// Persisted ledger tracking the atomic progress of restoring a BackupSet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreLedger {
    version: u32,
    set_id: String,
    manifest_digest: String,
    phase: RestorePhase,
    artifacts: Vec<ArtifactEntry>,
}

impl RestoreLedger {
    /// Initialize a new restore ledger for a BackupSet.
    ///
    /// Every artifact begins at [`ArtifactStage::Pending`], and the overall phase begins
    /// at [`RestorePhase::Staging`].
    pub fn begin(
        set_id: &str,
        manifest_digest: &str,
        artifacts: &[(String, u64)],
    ) -> Result<Self, RestoreLedgerError> {
        validate_set_id(set_id)?;
        validate_manifest_digest(manifest_digest)?;

        if artifacts.is_empty() {
            return Err(RestoreLedgerError::EmptyArtifacts);
        }
        if artifacts.len() > MAX_RESTORE_LEDGER_ARTIFACTS {
            return Err(RestoreLedgerError::TooManyArtifacts {
                count: artifacts.len(),
                max: MAX_RESTORE_LEDGER_ARTIFACTS,
            });
        }

        let mut seen = HashSet::with_capacity(artifacts.len());
        let mut entries = Vec::with_capacity(artifacts.len());
        for (name, size_bytes) in artifacts {
            if name.is_empty() {
                return Err(RestoreLedgerError::EmptyArtifactName);
            }
            if !seen.insert(name.as_str()) {
                return Err(RestoreLedgerError::DuplicateArtifact(name.clone()));
            }
            entries.push(ArtifactEntry {
                name: name.clone(),
                size_bytes: *size_bytes,
                stage: ArtifactStage::Pending,
            });
        }

        Ok(Self {
            version: 1,
            set_id: set_id.to_string(),
            manifest_digest: manifest_digest.to_string(),
            phase: RestorePhase::Staging,
            artifacts: entries,
        })
    }

    /// Check whether an existing ledger matches the incoming restore set.
    ///
    /// If the set ID and manifest digest match:
    /// - returns [`ResumePoint::AlreadyCommitted`] if the ledger is in [`RestorePhase::Committed`].
    /// - returns [`ResumePoint::Continue`] otherwise.
    ///
    /// If the set ID or manifest digest differs, returns [`RestoreLedgerError::ConflictingSet`]
    /// and leaves the ledger unchanged.
    pub fn resume(
        &self,
        set_id: &str,
        manifest_digest: &str,
    ) -> Result<ResumePoint, RestoreLedgerError> {
        if self.set_id != set_id {
            return Err(RestoreLedgerError::ConflictingSet {
                existing: self.set_id.clone(),
                attempted: set_id.to_string(),
            });
        }
        if self.manifest_digest != manifest_digest {
            return Err(RestoreLedgerError::ConflictingSet {
                existing: self.manifest_digest.clone(),
                attempted: manifest_digest.to_string(),
            });
        }

        if self.phase == RestorePhase::Committed {
            Ok(ResumePoint::AlreadyCommitted)
        } else {
            Ok(ResumePoint::Continue)
        }
    }

    /// Resume an existing restore ledger if present, or begin a fresh ledger.
    pub fn resume_or_begin(
        existing: Option<Self>,
        set_id: &str,
        manifest_digest: &str,
        artifacts: &[(String, u64)],
    ) -> Result<(Self, ResumePoint), RestoreLedgerError> {
        match existing {
            Some(ledger) => {
                let point = ledger.resume(set_id, manifest_digest)?;
                Ok((ledger, point))
            }
            None => {
                let ledger = Self::begin(set_id, manifest_digest, artifacts)?;
                Ok((ledger, ResumePoint::Fresh))
            }
        }
    }

    /// Mark an artifact verified (`Pending` -> `Verified`).
    ///
    /// Repeating on `Verified` or `Applied` is `Ok` and leaves the ledger unchanged.
    pub fn mark_verified(&mut self, name: &str) -> Result<(), RestoreLedgerError> {
        let entry = self
            .artifacts
            .iter_mut()
            .find(|a| a.name == name)
            .ok_or_else(|| RestoreLedgerError::UnknownArtifact(name.to_string()))?;

        match entry.stage {
            ArtifactStage::Pending => {
                if self.phase != RestorePhase::Staging {
                    return Err(RestoreLedgerError::IllegalPhaseTransition {
                        current: self.phase,
                        target: self.phase,
                        reason: format!(
                            "cannot verify artifact `{name}` in {:?} phase",
                            self.phase
                        ),
                    });
                }
                entry.stage = ArtifactStage::Verified;
                Ok(())
            }
            ArtifactStage::Verified | ArtifactStage::Applied => Ok(()),
        }
    }

    /// Transition from `Staging` to `Applying`.
    ///
    /// Every artifact must be at least `Verified`.
    /// Repeating while in `Applying` is `Ok`.
    pub fn begin_apply(&mut self) -> Result<(), RestoreLedgerError> {
        match self.phase {
            RestorePhase::Applying => Ok(()),
            RestorePhase::Committed => Err(RestoreLedgerError::IllegalPhaseTransition {
                current: self.phase,
                target: RestorePhase::Applying,
                reason: "cannot transition from Committed back to Applying".to_string(),
            }),
            RestorePhase::Staging => {
                let unverified: Vec<String> = self
                    .artifacts
                    .iter()
                    .filter(|a| a.stage == ArtifactStage::Pending)
                    .map(|a| a.name.clone())
                    .collect();
                if !unverified.is_empty() {
                    return Err(RestoreLedgerError::UnverifiedArtifacts(unverified));
                }
                self.phase = RestorePhase::Applying;
                Ok(())
            }
        }
    }

    /// Mark an artifact applied (`Verified` -> `Applied`).
    ///
    /// Allowed only while in `Applying`.
    /// Repeating on `Applied` is `Ok`.
    pub fn mark_applied(&mut self, name: &str) -> Result<(), RestoreLedgerError> {
        if self.phase != RestorePhase::Applying {
            return Err(RestoreLedgerError::IllegalPhaseTransition {
                current: self.phase,
                target: self.phase,
                reason: format!(
                    "cannot mark artifact applied while in {:?} phase (must be Applying)",
                    self.phase
                ),
            });
        }

        let entry = self
            .artifacts
            .iter_mut()
            .find(|a| a.name == name)
            .ok_or_else(|| RestoreLedgerError::UnknownArtifact(name.to_string()))?;

        match entry.stage {
            ArtifactStage::Verified => {
                entry.stage = ArtifactStage::Applied;
                Ok(())
            }
            ArtifactStage::Applied => Ok(()),
            ArtifactStage::Pending => Err(RestoreLedgerError::IllegalArtifactTransition(
                name.to_string(),
                ArtifactStage::Pending,
            )),
        }
    }

    /// Transition from `Applying` to `Committed`.
    ///
    /// Allowed only when every artifact has been `Applied`.
    /// Repeating on `Committed` is `Ok`.
    pub fn commit(&mut self) -> Result<(), RestoreLedgerError> {
        match self.phase {
            RestorePhase::Committed => Ok(()),
            RestorePhase::Staging => Err(RestoreLedgerError::IllegalPhaseTransition {
                current: self.phase,
                target: RestorePhase::Committed,
                reason: "cannot commit from Staging phase".to_string(),
            }),
            RestorePhase::Applying => {
                let unapplied: Vec<String> = self
                    .artifacts
                    .iter()
                    .filter(|a| a.stage != ArtifactStage::Applied)
                    .map(|a| a.name.clone())
                    .collect();
                if !unapplied.is_empty() {
                    return Err(RestoreLedgerError::UnappliedArtifacts(unapplied));
                }
                self.phase = RestorePhase::Committed;
                Ok(())
            }
        }
    }

    /// Return the current restore phase.
    pub fn phase(&self) -> RestorePhase {
        self.phase
    }

    /// Return the set ID.
    pub fn set_id(&self) -> &str {
        &self.set_id
    }

    /// Return the manifest digest.
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    /// Names of artifacts still `Pending`, in original manifest order.
    pub fn pending(&self) -> Vec<&str> {
        self.artifacts
            .iter()
            .filter(|a| a.stage == ArtifactStage::Pending)
            .map(|a| a.name.as_str())
            .collect()
    }

    /// Names of artifacts not yet `Applied` (either `Pending` or `Verified`), in original manifest order.
    pub fn unapplied(&self) -> Vec<&str> {
        self.artifacts
            .iter()
            .filter(|a| a.stage != ArtifactStage::Applied)
            .map(|a| a.name.as_str())
            .collect()
    }

    /// Return the stage of a given artifact, if present.
    pub fn artifact_stage(&self, name: &str) -> Option<ArtifactStage> {
        self.artifacts
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.stage)
    }

    /// Return the artifact entries in original manifest order.
    pub fn artifacts(&self) -> &[ArtifactEntry] {
        &self.artifacts
    }

    /// Serialize the ledger to compact JSON.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serialization of RestoreLedger cannot fail")
    }

    /// Deserialize and validate a restore ledger from JSON bytes.
    ///
    /// Refuses input over 1 MiB before parsing, unknown fields, unsupported versions,
    /// and any document that violates the invariants of `begin` or the phase.
    pub fn from_json(bytes: &[u8]) -> Result<Self, RestoreLedgerError> {
        if bytes.len() > MAX_RESTORE_LEDGER_BYTES {
            return Err(RestoreLedgerError::OversizedJson {
                size: bytes.len(),
                max: MAX_RESTORE_LEDGER_BYTES,
            });
        }

        let ledger: RestoreLedger = serde_json::from_slice(bytes)?;

        if ledger.version != 1 {
            return Err(RestoreLedgerError::UnsupportedVersion(ledger.version));
        }

        validate_set_id(&ledger.set_id)?;
        validate_manifest_digest(&ledger.manifest_digest)?;

        if ledger.artifacts.is_empty() {
            return Err(RestoreLedgerError::EmptyArtifacts);
        }
        if ledger.artifacts.len() > MAX_RESTORE_LEDGER_ARTIFACTS {
            return Err(RestoreLedgerError::TooManyArtifacts {
                count: ledger.artifacts.len(),
                max: MAX_RESTORE_LEDGER_ARTIFACTS,
            });
        }

        let mut seen = HashSet::with_capacity(ledger.artifacts.len());
        for entry in &ledger.artifacts {
            if entry.name.is_empty() {
                return Err(RestoreLedgerError::EmptyArtifactName);
            }
            if !seen.insert(entry.name.as_str()) {
                return Err(RestoreLedgerError::DuplicateArtifact(entry.name.clone()));
            }
        }

        match ledger.phase {
            RestorePhase::Staging => {
                for a in &ledger.artifacts {
                    if a.stage == ArtifactStage::Applied {
                        return Err(RestoreLedgerError::InconsistentPhase {
                            phase: ledger.phase,
                            details: format!("artifact `{}` is Applied in Staging phase", a.name),
                        });
                    }
                }
            }
            RestorePhase::Applying => {
                for a in &ledger.artifacts {
                    if a.stage == ArtifactStage::Pending {
                        return Err(RestoreLedgerError::InconsistentPhase {
                            phase: ledger.phase,
                            details: format!("artifact `{}` is Pending in Applying phase", a.name),
                        });
                    }
                }
            }
            RestorePhase::Committed => {
                for a in &ledger.artifacts {
                    if a.stage != ArtifactStage::Applied {
                        return Err(RestoreLedgerError::InconsistentPhase {
                            phase: ledger.phase,
                            details: format!(
                                "artifact `{}` is not Applied in Committed phase",
                                a.name
                            ),
                        });
                    }
                }
            }
        }

        Ok(ledger)
    }
}

/// Resume an existing restore ledger if present, or begin a fresh ledger.
pub fn resume_or_begin(
    existing: Option<RestoreLedger>,
    set_id: &str,
    manifest_digest: &str,
    artifacts: &[(String, u64)],
) -> Result<(RestoreLedger, ResumePoint), RestoreLedgerError> {
    RestoreLedger::resume_or_begin(existing, set_id, manifest_digest, artifacts)
}

fn validate_set_id(set_id: &str) -> Result<(), RestoreLedgerError> {
    if set_id.is_empty()
        || set_id.len() > MAX_SET_ID_BYTES
        || !set_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(RestoreLedgerError::InvalidSetId(set_id.to_string()));
    }
    Ok(())
}

fn validate_manifest_digest(digest: &str) -> Result<(), RestoreLedgerError> {
    if !digest.starts_with("sha256:") {
        return Err(RestoreLedgerError::InvalidManifestDigest(
            digest.to_string(),
        ));
    }
    let hex_part = &digest["sha256:".len()..];
    if hex_part.len() != 64
        || !hex_part
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(RestoreLedgerError::InvalidManifestDigest(
            digest.to_string(),
        ));
    }
    Ok(())
}
