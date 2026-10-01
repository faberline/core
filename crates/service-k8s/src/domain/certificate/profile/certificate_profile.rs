//! The validated profile, and the bounds it is validated against.

use std::time::Duration;

use super::{CertificateIdentity, ExtendedUsage, InstanceScope, ProfileError, Purpose};

/// A validated request shape: purpose, names, and lifetime bounds.
///
/// Construct with [`CertificateProfile::new`], which is fallible. There is no
/// public field-literal path on purpose — an unvalidated profile is exactly the
/// value this module exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateProfile {
    /// The scope this profile was validated against.
    ///
    /// Retained rather than merely consulted at construction: "validated" has to
    /// stay checkable downstream, and a reconciler that cannot ask which
    /// instance a profile belongs to has no way to refuse one that belongs to
    /// another (R7).
    scope: InstanceScope,
    purpose: Purpose,
    common_name: String,
    identity: CertificateIdentity,
    lifetime: Duration,
    renew_before: Duration,
    renew_jitter: Duration,
}

/// Floor and ceiling on a leaf's lifetime, mirroring the issuing pool's own
/// bounds (#3109 `max_leaf_lifetime_seconds`). Stated here too because the
/// controller should refuse an impossible profile locally rather than learn it
/// from a rejected CSR.
pub const MIN_LIFETIME_SECS: u64 = 300;
pub const MAX_LIFETIME_SECS: u64 = 604_800;
/// The renewal window must leave room for several failed attempts. Ten minutes
/// is roughly a dozen retries at the backoff ceiling.
pub const MIN_RENEW_BEFORE_SECS: u64 = 600;

/// Kubernetes-internal DNS suffixes. Same list as the issuing pool's default
/// `allowed_dns_suffixes` (#3109) — kept in sync by intent, checked here so a
/// profile fails before a CSR is submitted rather than after.
const CLUSTER_SUFFIXES: [&str; 2] = [".svc.cluster.local", ".svc"];

impl CertificateProfile {
    /// Validate a profile against the scope that will own it.
    pub fn new(
        scope: &InstanceScope,
        purpose: Purpose,
        common_name: impl Into<String>,
        identity: CertificateIdentity,
        lifetime: Duration,
        renew_before: Duration,
        renew_jitter: Duration,
    ) -> Result<Self, ProfileError> {
        if identity.dns_names.is_empty() {
            return Err(ProfileError::NoNames);
        }
        for name in &identity.dns_names {
            if !CLUSTER_SUFFIXES
                .iter()
                .any(|suffix| name.ends_with(suffix))
            {
                return Err(ProfileError::PublicDnsName { name: name.clone() });
            }
            // `.<namespace>.svc` is the segment Kubernetes itself uses to
            // separate tenants in DNS, so it is the segment worth checking. A
            // suffix match on the namespace alone would accept
            // `evil.lumen-prod.svc` for namespace `prod`.
            let namespaced = format!(".{}.svc", scope.namespace);
            if !name.contains(&namespaced) {
                return Err(ProfileError::ForeignDnsName {
                    name: name.clone(),
                    namespace: scope.namespace.clone(),
                });
            }
        }
        match (&identity.spiffe_uri, purpose) {
            (None, Purpose::Peer) => return Err(ProfileError::PeerNeedsSpiffeUri),
            (Some(uri), _) => {
                let prefix = scope.spiffe_prefix();
                if !uri.starts_with(&prefix) {
                    return Err(ProfileError::ForeignSpiffeUri {
                        uri: uri.clone(),
                        expected_prefix: prefix,
                    });
                }
            }
            (None, Purpose::Serving) => {}
        }
        let lifetime_secs = lifetime.as_secs();
        if !(MIN_LIFETIME_SECS..=MAX_LIFETIME_SECS).contains(&lifetime_secs) {
            return Err(ProfileError::LifetimeOutOfBounds {
                seconds: lifetime_secs,
            });
        }
        let renew_secs = renew_before.as_secs();
        if renew_secs < MIN_RENEW_BEFORE_SECS {
            return Err(ProfileError::RenewWindowTooNarrow {
                renew_before_secs: renew_secs,
            });
        }
        if renew_secs >= lifetime_secs {
            return Err(ProfileError::RenewWindowTooWide {
                renew_before_secs: renew_secs,
                lifetime_secs,
            });
        }
        if renew_jitter > renew_before {
            return Err(ProfileError::JitterExceedsWindow {
                jitter_secs: renew_jitter.as_secs(),
                renew_before_secs: renew_secs,
            });
        }
        Ok(Self {
            scope: scope.clone(),
            purpose,
            common_name: common_name.into(),
            identity,
            lifetime,
            renew_before,
            renew_jitter,
        })
    }

    /// The scope this profile was validated against.
    pub fn scope(&self) -> &InstanceScope {
        &self.scope
    }

    pub fn purpose(&self) -> Purpose {
        self.purpose
    }

    pub fn common_name(&self) -> &str {
        &self.common_name
    }

    pub fn identity(&self) -> &CertificateIdentity {
        &self.identity
    }

    pub fn lifetime(&self) -> Duration {
        self.lifetime
    }

    pub fn renew_before(&self) -> Duration {
        self.renew_before
    }

    pub fn renew_jitter(&self) -> Duration {
        self.renew_jitter
    }

    pub fn extended_key_usages(&self) -> &'static [ExtendedUsage] {
        self.purpose.extended_key_usages()
    }

    /// A stable digest of everything a reissue would change: purpose, names,
    /// and usages.
    ///
    /// This is what lets a restarted controller decide "the leaf on disk is
    /// still the one this profile asks for" without keeping any memory of
    /// having issued it (R4). It covers the certified content only — lifetime
    /// is not in it, because changing the renewal cadence is not a reason to
    /// throw away a valid identity.
    pub fn identity_digest(&self) -> String {
        let mut parts = vec![format!("purpose={}", self.purpose.as_str())];
        parts.push(format!("cn={}", self.common_name));
        let mut dns = self.identity.dns_names.clone();
        dns.sort();
        parts.push(format!("dns={}", dns.join(",")));
        parts.push(format!(
            "uri={}",
            self.identity.spiffe_uri.as_deref().unwrap_or("")
        ));
        let usages: Vec<&str> = self
            .extended_key_usages()
            .iter()
            .map(|u| u.as_str())
            .collect();
        parts.push(format!("eku={}", usages.join(",")));
        crate::certificate::digest::hex_sha256(parts.join("|").as_bytes())
    }
}
