use super::*;
use crate::certificate::profile::{
    CertificateIdentity, CertificateProfile, InstanceScope, Purpose,
};
use crate::domain::certificate::issuer::{IssuanceRequest, Issuer, IssuerId};
use crate::infrastructure::certificate::csr::RcgenCsrGenerator;
use serde_json::json;
use std::time::Duration;

mod token_source;

fn scope() -> InstanceScope {
    InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
}

fn request() -> IssuanceRequest {
    let profile = CertificateProfile::new(
        &scope(),
        Purpose::Peer,
        "lumen-0.lumen-headless.lumen.svc.cluster.local",
        CertificateIdentity {
            dns_names: vec!["lumen-0.lumen-headless.lumen.svc.cluster.local".into()],
            spiffe_uri: Some("spiffe://lumen-prod.svc.id.goog/ns/lumen/sa/lumen".into()),
        },
        Duration::from_secs(3_600),
        Duration::from_secs(900),
        Duration::from_secs(0),
    )
    .unwrap();
    IssuanceRequest::build(&scope(), &profile, &RcgenCsrGenerator)
        .unwrap()
        .0
}

fn pool() -> CaPool {
    CaPool {
        project: "axiom-prod".into(),
        location: "us-central1".into(),
        pool: "lumen-issuing".into(),
    }
}

struct StaticToken;
impl AccessTokenSource for StaticToken {
    fn token<'a>(&'a self) -> BoxFuture<'a, Result<String, IssuerError>> {
        Box::pin(futures::future::ready(Ok("test-token".to_string())))
    }
}

#[test]
fn a_pool_resource_name_round_trips() {
    let resource = "projects/axiom-prod/locations/us-central1/caPools/lumen-issuing";
    assert_eq!(CaPool::parse(resource).unwrap(), pool());
    assert_eq!(pool().resource(), resource);
}

#[test]
fn something_that_is_not_a_pool_is_rejected_rather_than_half_parsed() {
    for bad in [
        "projects/axiom-prod/locations/us-central1",
        "projects//locations/us-central1/caPools/lumen-issuing",
        "caPools/lumen-issuing",
        "",
    ] {
        assert!(CaPool::parse(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn the_request_body_asks_for_nothing_the_pool_would_refuse() {
    let body = CasIssuer::request_body(&request());
    let fields = body.as_object().unwrap();
    assert!(fields.contains_key("pemCsr"));
    assert_eq!(body["lifetime"], json!("3600s"));
    assert!(
        !fields.contains_key("config"),
        "the pool refuses config-based issuance (#3109); asking anyway turns a policy \
         into a runtime error"
    );
    assert!(!fields.contains_key("issuingCertificateAuthorityId"));
}

#[test]
fn the_same_csr_addresses_the_same_certificate() {
    let request = request();
    let first = CasIssuer::certificate_id(&request);
    let second = CasIssuer::certificate_id(&request);
    assert_eq!(
        first, second,
        "a retry after a timeout must re-address the certificate the CA may already have \
         issued, not mint a sibling"
    );
    assert!(first.starts_with("lumen-peer-"));
    assert!(first.len() <= 63);
}

#[test]
fn different_requests_do_not_collide() {
    assert_ne!(
        CasIssuer::certificate_id(&request()),
        CasIssuer::certificate_id(&request()),
        "each request carries a fresh key, so each addresses its own certificate"
    );
}

#[test]
fn the_url_names_the_pool_and_the_certificate() {
    let issuer = CasIssuer::new(pool(), Box::new(StaticToken));
    let url = issuer.certificates_url("lumen-peer-0123456789abcdef");
    assert_eq!(
        url,
        "https://privateca.googleapis.com/v1/projects/axiom-prod/locations/us-central1\
         /caPools/lumen-issuing/certificates?certificateId=lumen-peer-0123456789abcdef"
    );
}

#[test]
fn the_issuer_id_is_the_pool_it_issues_from() {
    let issuer = CasIssuer::new(pool(), Box::new(StaticToken));
    assert_eq!(issuer.id(), IssuerId::new(pool().resource()));
}

#[test]
fn a_response_is_believed_only_as_far_as_the_certificate_in_it() {
    // A response whose envelope is missing entirely still yields validity,
    // because validity is read from the leaf.
    let issuer = EphemeralHelper::material();
    let body = json!({
        "pemCertificate": issuer.0,
        "pemCertificateChain": [issuer.1],
    });
    let material = CasIssuer::material(&IssuerId::new("pool"), &body).unwrap();
    assert_eq!(material.not_after, issuer.2);
}

#[test]
fn a_response_without_a_certificate_is_an_error_not_an_empty_leaf() {
    let body = json!({ "pemCertificateChain": [] });
    assert!(CasIssuer::material(&IssuerId::new("pool"), &body).is_err());
}

#[test]
fn anchors_flatten_across_every_chain_the_pool_reports() {
    let body = json!({
        "caCerts": [
            { "certificates": ["-----BEGIN CERTIFICATE-----\nQQ==\n-----END CERTIFICATE-----"] },
            { "certificates": ["-----BEGIN CERTIFICATE-----\nQg==\n-----END CERTIFICATE-----"] },
        ]
    });
    let anchors = CasIssuer::anchors_from(&body);
    assert_eq!(anchors.matches("BEGIN CERTIFICATE").count(), 2);
}

#[test]
fn ca_pool_parse_rejects_unsafe_characters() {
    assert!(CaPool::parse("projects/p/locations/l/caPools/n\nvalue: inject").is_err());
    assert!(CaPool::parse("projects/p/locations/l/caPools/n\"injected").is_err());
    assert!(CaPool::parse("projects/p/locations/l/caPools/n\\injected").is_err());
}

/// Signs one leaf with the in-process CA so the response-parsing tests have
/// a real certificate to parse, without reaching a network.
struct EphemeralHelper;
impl EphemeralHelper {
    fn material() -> (String, String, chrono::DateTime<chrono::Utc>) {
        use crate::certificate::ephemeral::{instant, EphemeralIssuer};
        let issuer = EphemeralIssuer::new("pool", instant(2026, 7, 1, 12));
        let material = futures::executor::block_on(issuer.issue(request())).unwrap();
        (
            material.certificate_pem.clone(),
            material.chain_pem.clone(),
            material.not_after,
        )
    }
}
