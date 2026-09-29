//! The Secret a certificate is projected into: the objects written for a
//! leaf or a trust bundle, and the facts read back out of them.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Value};

use crate::domain::certificate::digest::hex_sha256;
use crate::domain::certificate::issuer::{IssuedMaterial, IssuerId};
use crate::domain::certificate::profile::{InstanceScope, Purpose};
use crate::domain::certificate::projection::{
    split_pem_blocks, Owner, ProjectedState, TrustBundle, CERT_KEY, IDENTITY_DIGEST_ANNOTATION,
    LEAF_ISSUER_ANNOTATION, PRIVATE_KEY_KEY, TRUST_BUNDLE_ANNOTATION, TRUST_BUNDLE_KEY,
};
use crate::domain::certificate::state::ObservedLeaf;

/// Labels every object this lifecycle writes carries, so a sweep can find them
/// and an operator can tell at a glance what created them.
fn labels(scope: &InstanceScope, purpose: Purpose) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), scope.instance.clone()),
        (
            "app.kubernetes.io/managed-by".to_string(),
            "service-k8s".to_string(),
        ),
        (
            "app.kubernetes.io/component".to_string(),
            format!("{}-tls", purpose.as_str()),
        ),
    ])
}

impl Owner {
    fn reference(&self) -> Value {
        json!({
            "apiVersion": self.api_version,
            "kind": self.kind,
            "name": self.name,
            "uid": self.uid,
            "controller": true,
            "blockOwnerDeletion": true,
        })
    }
}

/// Read a Secret's data back into the facts the state machine reasons about.
///
/// `data` is the decoded Secret data (the `kube` client hands out base64; the
/// caller decodes, because this module has no opinion about transport).
pub fn read_state(
    data: &BTreeMap<String, Vec<u8>>,
    annotations: &BTreeMap<String, String>,
) -> ProjectedState {
    let bundle = data
        .get(TRUST_BUNDLE_KEY)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map(|pem| {
            TrustBundle::parse(pem, annotations.get(TRUST_BUNDLE_ANNOTATION).map(String::as_str))
        })
        .unwrap_or_default();

    let leaf = data
        .get(CERT_KEY)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|pem| {
            let issuer = annotations.get(LEAF_ISSUER_ANNOTATION)?;
            let identity_digest = annotations.get(IDENTITY_DIGEST_ANNOTATION)?;
            let facts = parse_leaf(pem).ok()?;
            Some(ObservedLeaf {
                issuer: IssuerId::new(issuer.clone()),
                not_before: facts.not_before,
                not_after: facts.not_after,
                fingerprint: facts.fingerprint,
                identity_digest: identity_digest.clone(),
            })
        });

    ProjectedState { leaf, bundle }
}

/// Facts read from the leaf itself.
pub struct LeafFacts {
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub fingerprint: String,
}

/// Parse validity and fingerprint out of a PEM leaf.
pub fn parse_leaf(pem: &str) -> Result<LeafFacts, String> {
    let block = split_pem_blocks(pem)
        .into_iter()
        .next()
        .ok_or_else(|| "no PEM block".to_string())?;
    let der = pem_body_to_der(&block)?;
    let (_, cert) = x509_parser::parse_x509_certificate(&der)
        .map_err(|err| format!("parse certificate: {err}"))?;
    let not_before = Utc
        .timestamp_opt(cert.validity().not_before.timestamp(), 0)
        .single()
        .ok_or_else(|| "notBefore is not a representable instant".to_string())?;
    let not_after = Utc
        .timestamp_opt(cert.validity().not_after.timestamp(), 0)
        .single()
        .ok_or_else(|| "notAfter is not a representable instant".to_string())?;
    Ok(LeafFacts {
        not_before,
        not_after,
        fingerprint: hex_sha256(&der),
    })
}

fn pem_body_to_der(block: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let body: String = block
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|err| format!("decode PEM body: {err}"))
}

/// The Secret carrying a full set of material: leaf, key, and trust bundle.
///
/// `type` is `Opaque` rather than `kubernetes.io/tls` on purpose. The TLS type
/// requires both `tls.crt` and `tls.key` to be present at all times, which
/// would make [`trust_bundle_secret`] — the bootstrap step that publishes trust
/// *before* any leaf exists — unrepresentable. The keys are what consumers read;
/// the type is what would stop the sequence from having a first step.
pub fn material_secret(
    scope: &InstanceScope,
    purpose: Purpose,
    owner: &Owner,
    material: &IssuedMaterial,
    private_key_pem: &str,
    bundle: &TrustBundle,
    identity_digest: &str,
) -> Value {
    let mut secret = base_secret(scope, purpose, owner);
    secret["metadata"]["annotations"] = json!({
        TRUST_BUNDLE_ANNOTATION: bundle.annotation(),
        LEAF_ISSUER_ANNOTATION: material.issuer.as_str(),
        IDENTITY_DIGEST_ANNOTATION: identity_digest,
    });
    secret["stringData"] = json!({
        CERT_KEY: material.certificate_pem,
        PRIVATE_KEY_KEY: private_key_pem,
        TRUST_BUNDLE_KEY: bundle.to_pem(),
    });
    secret
}

/// The Secret carrying only a trust bundle.
///
/// Applied with a merge patch so it widens `ca.crt` without touching
/// `tls.crt`/`tls.key`. That is R5's "a failed step retains the last valid
/// serving material" at the point where it is easiest to get wrong: publishing
/// the next issuer's anchor must never be able to blank the leaf that is
/// currently serving traffic.
pub fn trust_bundle_secret(
    scope: &InstanceScope,
    purpose: Purpose,
    owner: &Owner,
    bundle: &TrustBundle,
) -> Value {
    let mut secret = base_secret(scope, purpose, owner);
    secret["metadata"]["annotations"] = json!({
        TRUST_BUNDLE_ANNOTATION: bundle.annotation(),
    });
    secret["stringData"] = json!({
        TRUST_BUNDLE_KEY: bundle.to_pem(),
    });
    secret
}

fn base_secret(scope: &InstanceScope, purpose: Purpose, owner: &Owner) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "type": "Opaque",
        "metadata": {
            "name": scope.secret_name(purpose),
            "namespace": scope.namespace,
            "labels": labels(scope, purpose),
            "ownerReferences": [owner.reference()],
        },
    })
}

#[cfg(test)]
mod tests;
