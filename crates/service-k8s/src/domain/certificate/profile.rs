//! What a service asks a certificate *for*.
//!
//! A profile is the service-specific half of the lifecycle: which names the
//! leaf must carry, which direction of TLS it is for, and how long it may
//! live. Everything else — issuing it, projecting it, renewing it, rotating
//! the issuer under it — is generic and lives in the sibling modules.
//!
//! The reason profiles are validated against an [`InstanceScope`] rather than
//! trusted is R7. A certificate is an authorization artifact: whoever can name
//! the identity on it can obtain that identity. If a profile could name
//! `lumen.other-tenant.svc.cluster.local`, then a bug in one instance's
//! reconcile loop is a cross-tenant impersonation, not a misconfiguration.
//! Checking here — before a key is generated and before any Secret is read —
//! is what makes AC4 a property of construction rather than of code review.

use std::fmt;

mod certificate_profile;

pub use certificate_profile::{
    CertificateProfile, MAX_LIFETIME_SECS, MIN_LIFETIME_SECS, MIN_RENEW_BEFORE_SECS,
};

/// Which direction of TLS a leaf is for.
///
/// This is not cosmetic. It selects the extended key usages, and the two
/// answers are genuinely different: a serving leaf that also carried
/// `clientAuth` could be replayed by whoever holds it to authenticate *as*
/// the service to its own peers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Client-facing TLS. Answers to Service DNS names; never authenticates
    /// outward.
    Serving,
    /// Peer mTLS. Both ends of a Raft link present one of these to each other,
    /// so it is server and client at once.
    Peer,
}

impl Purpose {
    /// Stable lowercase token used in Secret names, metrics, and conditions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Serving => "serving",
            Self::Peer => "peer",
        }
    }

    /// The extended key usages a leaf of this purpose may carry — exactly, not
    /// at least. `Serving` deliberately omits `clientAuth`.
    pub fn extended_key_usages(self) -> &'static [ExtendedUsage] {
        match self {
            Self::Serving => &[ExtendedUsage::ServerAuth],
            Self::Peer => &[ExtendedUsage::ServerAuth, ExtendedUsage::ClientAuth],
        }
    }
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The extended key usages this lifecycle can express. There is no `Any`, and
/// no code-signing or OCSP variant: a leaf minted here is for one Kubernetes
/// service talking to another, and a wider enum would be a wider blast radius
/// for a typo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtendedUsage {
    ServerAuth,
    ClientAuth,
}

impl ExtendedUsage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ServerAuth => "serverAuth",
            Self::ClientAuth => "clientAuth",
        }
    }
}

/// One Lumen instance, in one namespace. Every read, write, owner reference,
/// and issuance request in this lifecycle is scoped by one of these.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InstanceScope {
    pub namespace: String,
    pub instance: String,
    /// The environment's single SPIFFE trust domain (#3109 R5). Identities are
    /// scoped inside it; it is not per-instance.
    pub trust_domain: String,
}

impl InstanceScope {
    pub fn new(
        namespace: impl Into<String>,
        instance: impl Into<String>,
        trust_domain: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            instance: instance.into(),
            trust_domain: trust_domain.into(),
        }
    }

    /// Where this instance's material for `purpose` lives. Derived, never
    /// supplied: a caller-chosen Secret name is a caller-chosen place to write,
    /// and this lifecycle owns what it writes.
    pub fn secret_name(&self, purpose: Purpose) -> String {
        format!("{}-{}-tls", self.instance, purpose.as_str())
    }

    /// The SPIFFE identity prefix every leaf in this scope must sit under.
    pub fn spiffe_prefix(&self) -> String {
        format!("spiffe://{}/ns/{}/", self.trust_domain, self.namespace)
    }

    /// True when `other` is the same instance in the same namespace of the same
    /// trust domain.
    pub fn covers(&self, other: &InstanceScope) -> bool {
        self == other
    }
}

/// Names a leaf must carry.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CertificateIdentity {
    /// Kubernetes-internal DNS names. Public names are not expressible: the
    /// issuing pool would refuse them (#3109 AC3), and this refuses them first.
    pub dns_names: Vec<String>,
    /// SPIFFE URI SAN. Required for peer leaves — it is what a peer actually
    /// authorizes against, since DNS alone cannot distinguish two members of
    /// the same headless Service from each other's point of view.
    pub spiffe_uri: Option<String>,
}

/// Why a profile was refused. Every variant names the offending value: an
/// operator reading this in a condition should not have to diff two lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    NoNames,
    ForeignDnsName {
        name: String,
        namespace: String,
    },
    PublicDnsName {
        name: String,
    },
    ForeignSpiffeUri {
        uri: String,
        expected_prefix: String,
    },
    PeerNeedsSpiffeUri,
    LifetimeOutOfBounds {
        seconds: u64,
    },
    RenewWindowTooWide {
        renew_before_secs: u64,
        lifetime_secs: u64,
    },
    RenewWindowTooNarrow {
        renew_before_secs: u64,
    },
    JitterExceedsWindow {
        jitter_secs: u64,
        renew_before_secs: u64,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoNames => write!(f, "a certificate profile must request at least one DNS name"),
            Self::ForeignDnsName { name, namespace } => write!(
                f,
                "DNS name {name} is not inside namespace {namespace}; one instance may not \
                 request an identity belonging to another"
            ),
            Self::PublicDnsName { name } => write!(
                f,
                "DNS name {name} is not a Kubernetes-internal name; this trust domain does not \
                 issue publicly resolvable identities"
            ),
            Self::ForeignSpiffeUri {
                uri,
                expected_prefix,
            } => write!(
                f,
                "SPIFFE URI {uri} is outside this instance's scope; it must begin with \
                 {expected_prefix}"
            ),
            Self::PeerNeedsSpiffeUri => write!(
                f,
                "a peer profile must carry a SPIFFE URI: DNS alone cannot distinguish two \
                 members of the same headless Service"
            ),
            Self::LifetimeOutOfBounds { seconds } => write!(
                f,
                "leaf lifetime {seconds}s is outside {MIN_LIFETIME_SECS}s..{MAX_LIFETIME_SECS}s; \
                 shorter cannot survive a controller outage, longer stops being short-lived"
            ),
            Self::RenewWindowTooWide {
                renew_before_secs,
                lifetime_secs,
            } => write!(
                f,
                "renew_before {renew_before_secs}s is not shorter than the {lifetime_secs}s \
                 lifetime; a leaf due for renewal the moment it is issued renews forever"
            ),
            Self::RenewWindowTooNarrow { renew_before_secs } => write!(
                f,
                "renew_before {renew_before_secs}s leaves no room to retry a failed issuance \
                 before the current leaf expires; the floor is {MIN_RENEW_BEFORE_SECS}s"
            ),
            Self::JitterExceedsWindow {
                jitter_secs,
                renew_before_secs,
            } => write!(
                f,
                "renew jitter {jitter_secs}s exceeds the {renew_before_secs}s renewal window; \
                 spreading renewals must not push one past expiry"
            ),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests;
