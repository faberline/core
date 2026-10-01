use std::time::Duration;

use super::*;
use crate::domain::certificate::issuer::IssuanceRequest;
use crate::domain::certificate::profile::{CertificateIdentity, InstanceScope, Purpose};

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

#[test]
fn a_csr_carries_the_requested_names() {
    let (request, _key) =
        IssuanceRequest::build(&scope(), &peer_profile(), &RcgenCsrGenerator).unwrap();
    let parsed = rcgen::CertificateSigningRequestParams::from_pem(&request.csr_pem).unwrap();
    let names: Vec<String> = parsed
        .params
        .subject_alt_names
        .iter()
        .map(|san| format!("{san:?}"))
        .collect();
    let joined = names.join(" ");
    assert!(
        joined.contains("lumen-0.lumen-headless.lumen.svc.cluster.local"),
        "got {joined}"
    );
    assert!(
        joined.contains("spiffe://lumen-prod.svc.id.goog/ns/lumen/sa/lumen"),
        "got {joined}"
    );
}

#[test]
fn a_csr_never_asks_to_be_a_ca() {
    let (request, _key) =
        IssuanceRequest::build(&scope(), &peer_profile(), &RcgenCsrGenerator).unwrap();
    let parsed = rcgen::CertificateSigningRequestParams::from_pem(&request.csr_pem).unwrap();
    assert_eq!(parsed.params.is_ca, rcgen::IsCa::NoCa);
    assert!(!parsed
        .params
        .key_usages
        .contains(&rcgen::KeyUsagePurpose::KeyCertSign));
    assert!(!parsed
        .params
        .key_usages
        .contains(&rcgen::KeyUsagePurpose::CrlSign));
}

#[test]
fn the_private_key_is_not_in_the_csr() {
    let (request, _key) =
        IssuanceRequest::build(&scope(), &peer_profile(), &RcgenCsrGenerator).unwrap();
    // The property that makes delegated issuance safe at all: what goes to
    // the CA proves possession of the key without containing it.
    assert!(!request.csr_pem.contains("PRIVATE KEY"));
    assert!(request.csr_pem.contains("BEGIN CERTIFICATE REQUEST"));
}

#[test]
fn every_request_gets_a_fresh_key() {
    let a = IssuanceRequest::build(&scope(), &peer_profile(), &RcgenCsrGenerator)
        .unwrap()
        .1
        .into_pem();
    let b = IssuanceRequest::build(&scope(), &peer_profile(), &RcgenCsrGenerator)
        .unwrap()
        .1
        .into_pem();
    assert_ne!(a, b, "renewal must not reuse the key it is replacing");
}
