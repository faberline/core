//! Where the material lives, and how it is read back.
//!
//! One Secret per purpose per instance, carrying the same three keys #2890
//! already projects — `tls.crt`, `tls.key`, `ca.crt`. Keeping that layout is
//! deliberate: the consumer contract for peer material is already deployed, and
//! a lifecycle that changed it would have to change the pod spec too, which is
//! the one thing R9 says renewal must never require.
//!
//! `ca.crt` is the trust *bundle*, not a single anchor. During a rotation it
//! holds the outgoing and incoming issuers at once, which is what makes the
//! overlap in [`super::state`] mean anything: a verifier reading this file
//! accepts both while the fleet crosses over. `rustls`' root store — and so
//! `peer_tls::PeerTlsConfig` — reads every PEM block in the file, so this needs
//! no consumer change either.
//!
//! ### Which facts are read from where
//!
//! Expiry and fingerprint are parsed from the certificate itself. Issuer ids
//! and the identity digest come from annotations, because they are not
//! derivable from the DER — an issuer id is our name for a pool, not the
//! subject on the chain.
//!
//! That split is the honest one. An annotation is a claim; a certificate is
//! evidence. Anywhere both could answer, the certificate answers, so a
//! hand-edited annotation cannot talk the controller into believing a leaf
//! expires later than it does.

use super::issuer::IssuerId;
use super::state::ObservedLeaf;

/// The three keys, in the order an operator would look for them.
pub const CERT_KEY: &str = "tls.crt";
pub const PRIVATE_KEY_KEY: &str = "tls.key";
pub const TRUST_BUNDLE_KEY: &str = "ca.crt";

/// Annotation carrying the ordered issuer ids whose anchors are in `ca.crt`.
pub const TRUST_BUNDLE_ANNOTATION: &str = "service-k8s.axiom.dev/trust-bundle";
/// Annotation naming the issuer that signed the leaf in `tls.crt`.
pub const LEAF_ISSUER_ANNOTATION: &str = "service-k8s.axiom.dev/leaf-issuer";
/// Annotation carrying the profile identity digest the leaf was issued for.
pub const IDENTITY_DIGEST_ANNOTATION: &str = "service-k8s.axiom.dev/identity-digest";

/// The owning custom resource. Every Secret this lifecycle writes is garbage
/// collected with it — R7's "scoped to one instance" applies to cleanup too,
/// and an orphaned Secret full of key material is exactly the kind of residue
/// nobody notices until an audit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub uid: String,
}

/// An ordered set of issuer anchors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustBundle {
    entries: Vec<(IssuerId, String)>,
}

impl TrustBundle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an anchor, replacing any previous PEM for the same issuer.
    pub fn insert(&mut self, issuer: IssuerId, anchor_pem: impl Into<String>) {
        let pem = anchor_pem.into();
        match self.entries.iter_mut().find(|(id, _)| *id == issuer) {
            Some(entry) => entry.1 = pem,
            None => self.entries.push((issuer, pem)),
        }
        self.entries.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// Keep only `issuers`. Used by the retire step, never by publish.
    pub fn retain(&mut self, issuers: &[IssuerId]) {
        self.entries.retain(|(id, _)| issuers.contains(id));
    }

    pub fn issuers(&self) -> Vec<IssuerId> {
        self.entries.iter().map(|(id, _)| id.clone()).collect()
    }

    pub fn contains(&self, issuer: &IssuerId) -> bool {
        self.entries.iter().any(|(id, _)| id == issuer)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Concatenated PEM, one anchor after another — what lands in `ca.crt`.
    pub fn to_pem(&self) -> String {
        let mut out = String::new();
        for (_, pem) in &self.entries {
            out.push_str(pem.trim_end());
            out.push('\n');
        }
        out
    }

    /// Rebuild from a Secret's `ca.crt` plus the issuer-id annotation.
    ///
    /// A count mismatch between the two is not repaired silently — it returns
    /// an empty bundle, which the state machine reads as "trust is not
    /// published", so the next reconcile republishes from the issuers
    /// themselves. Guessing which block belongs to which id would be a guess
    /// about what the fleet currently trusts.
    pub fn parse(pem: &str, annotation: Option<&str>) -> Self {
        let blocks = split_pem_blocks(pem);
        let ids: Vec<IssuerId> = annotation
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(IssuerId::new)
            .collect();
        if blocks.len() != ids.len() {
            return Self::default();
        }
        let mut bundle = Self::default();
        for (id, block) in ids.into_iter().zip(blocks) {
            bundle.insert(id, block);
        }
        bundle
    }

    pub(crate) fn annotation(&self) -> String {
        self.issuers()
            .iter()
            .map(IssuerId::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Split concatenated PEM into its individual blocks, preserving each one's
/// text exactly.
pub(crate) fn split_pem_blocks(pem: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in pem.lines() {
        if line.starts_with("-----BEGIN") {
            current = Some(vec![line]);
        } else if line.starts_with("-----END") {
            if let Some(mut lines) = current.take() {
                lines.push(line);
                blocks.push(lines.join("\n"));
            }
        } else if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }
    blocks
}

/// What a reconcile read out of the cluster.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectedState {
    pub leaf: Option<ObservedLeaf>,
    pub bundle: TrustBundle,
}
