use std::fmt;

/// Why a candidate was refused.
///
/// One variant per failure an operator can actually cause, because the reason is
/// what reaches the status surface (R6) and "reload failed" is not an
/// actionable thing to read at three in the morning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// A source file could not be read at all.
    Unreadable,
    /// PEM that does not decode, or a bundle with no certificate in it.
    MalformedPem,
    /// A trust bundle with no anchors: activating it would trust nothing and
    /// reject every peer, which is an outage rather than a rotation.
    EmptyTrustBundle,
    /// The private key does not belong to the leaf.
    KeyMismatch,
    /// `notBefore` is in the future.
    NotYetValid,
    /// `notAfter` is in the past.
    Expired,
    /// The leaf lacks the extended key usage the role requires.
    MissingUsage,
    /// The leaf does not chain to any anchor in the trust bundle.
    Untrusted,
    /// The leaf chains and is in date, but does not carry the DNS or SPIFFE
    /// identity this runtime is configured to present.
    WrongIdentity,
}

impl RejectionReason {
    /// The stable machine-readable spelling, for metrics labels and status.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::MalformedPem => "malformed_pem",
            Self::EmptyTrustBundle => "empty_trust_bundle",
            Self::KeyMismatch => "key_mismatch",
            Self::NotYetValid => "not_yet_valid",
            Self::Expired => "expired",
            Self::MissingUsage => "missing_usage",
            Self::Untrusted => "untrusted",
            Self::WrongIdentity => "wrong_identity",
        }
    }
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A refusal, with a detail string safe to log.
///
/// `detail` is built only from rustls' own error text, expected identity names,
/// and counts. It never carries PEM bodies or filesystem paths, because this
/// string ends up in request-adjacent logs and status conditions (#3112 R6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub reason: RejectionReason,
    pub detail: String,
}

impl Rejection {
    pub fn new(reason: RejectionReason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason, self.detail)
    }
}

impl std::error::Error for Rejection {}
