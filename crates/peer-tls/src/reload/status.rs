/// The bounded status surface: generation, fingerprint, expiry, counters, and
/// the last refusal.
///
/// Everything here is a number, a stable enum spelling, or a fingerprint. There
/// is no field a PEM body or a filesystem path can travel in, which is the point
/// — this is what reaches metrics and health, and those are read by things that
/// are not allowed to learn key material (#3112 R6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsReloadStatus {
    /// Increments once per successful activation; `0` means nothing ever
    /// activated.
    pub generation: u64,
    /// Lowercase hex sha256 of the active leaf, in the certificate controller's
    /// own spelling, so "did the runtime pick it up" is a string comparison.
    pub fingerprint: Option<String>,
    /// Seconds until the active leaf expires; `Some(0)` means expired.
    pub seconds_to_expiry: Option<u64>,
    pub accepted_reloads: u64,
    pub rejected_reloads: u64,
    /// Stable spelling of why the most recent refusal happened.
    pub last_error_reason: Option<&'static str>,
    /// Human-readable detail for the same refusal.
    pub last_error: Option<String>,
    pub trust_anchors: usize,
    /// Anchors retained from the previous generation, still accepted.
    pub retiring_trust_anchors: usize,
    /// Whether a handshake attempted right now would get a configuration.
    pub serving: bool,
}
