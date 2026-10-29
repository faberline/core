//! Streaming artifact verifier that computes running SHA-256 and validates declared size and digest.

use super::{BackupArtifact, BackupSetError};
use sha2::{Digest, Sha256};

/// Verifies a single artifact's byte stream chunk by chunk without buffering payload bytes in memory.
#[derive(Debug)]
pub struct ArtifactVerifier {
    expected_size: u64,
    expected_digest: String,
    bytes_seen: u64,
    hasher: Sha256,
}

impl ArtifactVerifier {
    /// Creates a new verifier for the given [`BackupArtifact`].
    pub fn new(artifact: &BackupArtifact) -> Self {
        Self {
            expected_size: artifact.size_bytes,
            expected_digest: artifact.digest.clone(),
            bytes_seen: 0,
            hasher: Sha256::new(),
        }
    }

    /// Feeds a chunk of bytes to the verifier.
    ///
    /// Fails immediately with [`BackupSetError::BytesExceeded`] if the accumulated
    /// bytes seen exceed the declared `sizeBytes`.
    pub fn update(&mut self, chunk: &[u8]) -> Result<(), BackupSetError> {
        self.bytes_seen = self.bytes_seen.saturating_add(chunk.len() as u64);
        if self.bytes_seen > self.expected_size {
            return Err(BackupSetError::BytesExceeded {
                expected: self.expected_size,
                actual: self.bytes_seen,
            });
        }
        self.hasher.update(chunk);
        Ok(())
    }

    /// Completes verification and checks that the total byte size and digest match.
    ///
    /// Fails with [`BackupSetError::ShortSize`] if fewer bytes were observed than declared,
    /// or [`BackupSetError::DigestMismatch`] if the computed SHA-256 does not match.
    pub fn finish(self) -> Result<(), BackupSetError> {
        if self.bytes_seen < self.expected_size {
            return Err(BackupSetError::ShortSize {
                expected: self.expected_size,
                actual: self.bytes_seen,
            });
        }

        let result = self.hasher.finalize();
        let computed = format!("sha256:{:x}", result);
        if computed != self.expected_digest {
            return Err(BackupSetError::DigestMismatch {
                expected: self.expected_digest,
                actual: computed,
            });
        }

        Ok(())
    }
}
