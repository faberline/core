use super::*;
use chrono::TimeZone;

fn facts() -> CertificateFacts {
    CertificateFacts {
        purpose: Purpose::Peer,
        issuer: Some(IssuerId::new("pool-a")),
        not_after: Some(Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap()),
        fingerprint: Some("0123456789abcdef".into()),
        trust_bundle: vec![IssuerId::new("pool-a")],
        rotating: None,
        consecutive_failures: 0,
    }
}

#[test]
fn a_pem_block_never_survives_into_status() {
    let leaked = format!(
        "issued {}",
        "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBg\n-----END PRIVATE KEY-----"
    );
    let cleaned = redact(&leaked);
    assert!(!cleaned.contains("MIIEvQIBADANBg"));
    assert!(!cleaned.contains("BEGIN PRIVATE KEY"));
    assert!(cleaned.contains("[redacted pem]"));
}

#[test]
fn a_bearer_token_never_survives_into_status() {
    let cleaned = redact("upstream said 401 for Bearer ya29.a0AfB_secretvalue here");
    assert!(!cleaned.contains("ya29.a0AfB_secretvalue"));
    assert!(cleaned.contains("[redacted]"));
}

#[test]
fn a_projected_token_never_survives_into_status() {
    let cleaned = redact("request carried eyJhbGciOiJSUzI1NiJ9.eyJhdWQiOlsibHVtZW4iXX0.c2lnbmF0dXJl as audience proof");
    assert!(!cleaned.contains("eyJhbGciOiJSUzI1NiJ9"));
    assert!(cleaned.contains("[redacted token]"));
}

#[test]
fn ordinary_text_passes_through_unharmed() {
    let text = "issuer pool-a; expires 2026-08-01T00:00:00+00:00";
    assert_eq!(redact(text), text);
}

#[test]
fn a_projected_certificate_reports_ready() {
    let conditions = facts().conditions();
    let ready = &conditions[0];
    assert_eq!(ready.type_, "PeerCertificateReady");
    assert_eq!(ready.status, ConditionStatus::True);
    assert!(ready.message.contains("pool-a"));
    assert!(ready.message.contains("expires"));
}

#[test]
fn a_rotation_does_not_make_the_instance_unready() {
    let mut facts = facts();
    facts.rotating = Some(IssueReason::Renewal);
    let conditions = facts.conditions();
    assert_eq!(conditions[0].status, ConditionStatus::True);
    assert_eq!(conditions[1].type_, "PeerCertificateRotating");
    assert_eq!(conditions[1].status, ConditionStatus::True);
    assert_eq!(conditions[1].reason, "Renewal");
}

#[test]
fn a_failing_lifecycle_says_so_rather_than_staying_pending() {
    let facts = CertificateFacts {
        issuer: None,
        not_after: None,
        fingerprint: None,
        consecutive_failures: 4,
        ..facts()
    };
    let conditions = facts.conditions();
    assert_eq!(conditions[0].status, ConditionStatus::False);
    assert_eq!(conditions[0].reason, "IssuanceFailing");
    assert!(conditions[0].message.contains('4'));
}

#[test]
fn serving_and_peer_conditions_do_not_collide() {
    let serving = CertificateFacts {
        purpose: Purpose::Serving,
        ..facts()
    };
    assert_eq!(serving.conditions()[0].type_, "ServingCertificateReady");
    assert_eq!(facts().conditions()[0].type_, "PeerCertificateReady");
}

#[test]
fn the_published_fingerprint_is_a_correlator_not_an_artifact() {
    let facts = CertificateFacts::from_action(
        Purpose::Serving,
        Some(IssuerId::new("pool-a")),
        None,
        Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        Vec::new(),
        0,
        &Action::Wait {
            until: Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap(),
        },
    );
    assert_eq!(facts.fingerprint.as_deref(), Some("0123456789abcdef"));
}
