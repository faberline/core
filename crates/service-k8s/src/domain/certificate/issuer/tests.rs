use std::time::Duration;

use super::*;
use crate::domain::certificate::profile::CertificateIdentity;

fn scope() -> InstanceScope {
    InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
}

fn peer_profile() -> CertificateProfile {
    CertificateProfile::new(
        &scope(),
        Purpose::Peer,
        "lumen-0.lumen-headless.lumen.svc.cluster.local",
        CertificateIdentity {
            dns_names: vec!["lumen-0.lumen-headless.lumen.svc.cluster.local".into()],
            spiffe_uri: Some("spiffe://lumen-prod.svc.id.goog/ns/lumen/sa/lumen".into()),
        },
        Duration::from_secs(86_400),
        Duration::from_secs(21_600),
        Duration::from_secs(1_800),
    )
    .unwrap()
}

/// A generator that hands back fixed text, so the test sees exactly what
/// `build` does with it.
struct FixedKeys;

impl KeyAndCsrGenerator for FixedKeys {
    fn generate(&self, profile: &CertificateProfile) -> Result<(String, String), IssuerError> {
        Ok((
            "KEY".to_string(),
            format!("CSR for {}", profile.common_name()),
        ))
    }
}

struct BrokenKeys;

impl KeyAndCsrGenerator for BrokenKeys {
    fn generate(&self, _profile: &CertificateProfile) -> Result<(String, String), IssuerError> {
        Err(IssuerError::KeyGeneration("no entropy".to_string()))
    }
}

#[test]
fn build_puts_the_csr_in_the_request_and_hands_the_key_back_apart() {
    let (request, key) = IssuanceRequest::build(&scope(), &peer_profile(), &FixedKeys).unwrap();
    assert_eq!(
        request.csr_pem,
        "CSR for lumen-0.lumen-headless.lumen.svc.cluster.local"
    );
    assert_eq!(request.purpose, Purpose::Peer);
    assert_eq!(
        request.spiffe_uri.as_deref(),
        Some("spiffe://lumen-prod.svc.id.goog/ns/lumen/sa/lumen")
    );
    assert_eq!(request.lifetime, Duration::from_secs(86_400));
    assert_eq!(key.into_pem(), "KEY");
}

#[test]
fn a_generator_failure_is_the_build_failure() {
    let Err(err) = IssuanceRequest::build(&scope(), &peer_profile(), &BrokenKeys) else {
        panic!("a failed key generation must not produce a request");
    };
    assert_eq!(err.to_string(), "generate key and CSR: no entropy");
}
