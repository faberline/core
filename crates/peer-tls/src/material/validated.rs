use std::fmt;
use std::time::SystemTime;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Material that has proved everything in [`IdentityExpectation`](super::IdentityExpectation) and would
/// survive a handshake at the instant it was validated.
pub struct ValidatedMaterial {
    pub(super) chain: Vec<CertificateDer<'static>>,
    pub(super) key: PrivateKeyDer<'static>,
    pub(super) trust: Vec<CertificateDer<'static>>,
    pub(super) fingerprint: String,
    pub(super) not_before: SystemTime,
    pub(super) not_after: SystemTime,
}

impl fmt::Debug for ValidatedMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedMaterial")
            .field("fingerprint", &self.fingerprint)
            .field("chain_len", &self.chain.len())
            .field("trust_anchors", &self.trust.len())
            .finish_non_exhaustive()
    }
}

impl ValidatedMaterial {
    /// Lowercase hex sha256 of the leaf DER — the same spelling the certificate
    /// controller writes into status, so "did the runtime pick up the leaf I
    /// issued?" is a string comparison and not a guess (#3110, #3112 R5).
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn not_before(&self) -> SystemTime {
        self.not_before
    }

    pub fn not_after(&self) -> SystemTime {
        self.not_after
    }

    /// The leaf and any intermediates, end-entity first.
    pub fn chain(&self) -> &[CertificateDer<'static>] {
        &self.chain
    }

    /// The anchors this material was validated against.
    pub fn trust_anchors(&self) -> &[CertificateDer<'static>] {
        &self.trust
    }

    pub fn key(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    /// Whether this material is still in date at `now`.
    pub fn is_valid_at(&self, now: SystemTime) -> bool {
        now >= self.not_before && now < self.not_after
    }

    /// Whole seconds until expiry, saturating at zero once expired.
    pub fn seconds_to_expiry(&self, now: SystemTime) -> u64 {
        self.not_after
            .duration_since(now)
            .map(|left| left.as_secs())
            .unwrap_or(0)
    }
}
