use super::*;
use std::time::Duration;

fn scope() -> InstanceScope {
    InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
}

fn serving_identity() -> CertificateIdentity {
    CertificateIdentity {
        dns_names: vec!["lumen.lumen.svc.cluster.local".into(), "lumen.lumen.svc".into()],
        spiffe_uri: None,
    }
}

fn build(
    purpose: Purpose,
    identity: CertificateIdentity,
) -> Result<CertificateProfile, ProfileError> {
    CertificateProfile::new(
        &scope(),
        purpose,
        "lumen.lumen.svc.cluster.local",
        identity,
        Duration::from_secs(86_400),
        Duration::from_secs(21_600),
        Duration::from_secs(1_800),
    )
}

#[test]
fn serving_leaves_do_not_carry_client_auth() {
    let profile = build(Purpose::Serving, serving_identity()).unwrap();
    assert_eq!(
        profile.extended_key_usages(),
        &[ExtendedUsage::ServerAuth],
        "a serving leaf that could also authenticate outward is a credential its holder \
         can replay against the service's own peers"
    );
}

#[test]
fn peer_leaves_carry_both_directions() {
    let identity = CertificateIdentity {
        dns_names: vec!["lumen-0.lumen-headless.lumen.svc.cluster.local".into()],
        spiffe_uri: Some("spiffe://lumen-prod.svc.id.goog/ns/lumen/sa/lumen".into()),
    };
    let profile = build(Purpose::Peer, identity).unwrap();
    assert_eq!(
        profile.extended_key_usages(),
        &[ExtendedUsage::ServerAuth, ExtendedUsage::ClientAuth]
    );
}

#[test]
fn a_peer_profile_without_a_spiffe_uri_is_refused() {
    let identity = CertificateIdentity {
        dns_names: vec!["lumen-0.lumen-headless.lumen.svc.cluster.local".into()],
        spiffe_uri: None,
    };
    assert_eq!(
        build(Purpose::Peer, identity),
        Err(ProfileError::PeerNeedsSpiffeUri)
    );
}

#[test]
fn another_namespaces_dns_name_is_refused() {
    let identity = CertificateIdentity {
        dns_names: vec!["lumen.other-tenant.svc.cluster.local".into()],
        spiffe_uri: None,
    };
    assert!(matches!(
        build(Purpose::Serving, identity),
        Err(ProfileError::ForeignDnsName { .. })
    ));
}

#[test]
fn a_namespace_prefix_is_not_a_namespace_match() {
    // `lumen-prod` starts with `lumen`, and a suffix or prefix check would
    // wave this through for the `lumen` namespace.
    let identity = CertificateIdentity {
        dns_names: vec!["lumen.lumen-prod.svc.cluster.local".into()],
        spiffe_uri: None,
    };
    assert!(matches!(
        build(Purpose::Serving, identity),
        Err(ProfileError::ForeignDnsName { .. })
    ));
}

#[test]
fn a_public_dns_name_is_refused() {
    let identity = CertificateIdentity {
        dns_names: vec!["lumen.example.com".into()],
        spiffe_uri: None,
    };
    assert!(matches!(
        build(Purpose::Serving, identity),
        Err(ProfileError::PublicDnsName { .. })
    ));
}

#[test]
fn another_namespaces_spiffe_identity_is_refused() {
    let identity = CertificateIdentity {
        dns_names: vec!["lumen.lumen.svc.cluster.local".into()],
        spiffe_uri: Some("spiffe://lumen-prod.svc.id.goog/ns/other-tenant/sa/lumen".into()),
    };
    assert!(matches!(
        build(Purpose::Serving, identity),
        Err(ProfileError::ForeignSpiffeUri { .. })
    ));
}

#[test]
fn a_renewal_window_with_no_room_to_retry_is_refused() {
    let err = CertificateProfile::new(
        &scope(),
        Purpose::Serving,
        "lumen.lumen.svc.cluster.local",
        serving_identity(),
        Duration::from_secs(86_400),
        Duration::from_secs(60),
        Duration::ZERO,
    );
    assert!(matches!(
        err,
        Err(ProfileError::RenewWindowTooNarrow { .. })
    ));
}

#[test]
fn identity_digest_ignores_dns_name_order_but_not_content() {
    let a = build(Purpose::Serving, serving_identity()).unwrap();
    let mut reordered = serving_identity();
    reordered.dns_names.reverse();
    let b = build(Purpose::Serving, reordered).unwrap();
    assert_eq!(
        a.identity_digest(),
        b.identity_digest(),
        "reordering the same names is not a reissue"
    );

    let extra = CertificateIdentity {
        dns_names: vec![
            "lumen.lumen.svc.cluster.local".into(),
            "lumen.lumen.svc".into(),
            "lumen-read.lumen.svc.cluster.local".into(),
        ],
        spiffe_uri: None,
    };
    let c = build(Purpose::Serving, extra).unwrap();
    assert_ne!(a.identity_digest(), c.identity_digest());
}

#[test]
fn secret_names_are_derived_from_the_scope_not_supplied() {
    let scope = scope();
    assert_eq!(scope.secret_name(Purpose::Serving), "lumen-serving-tls");
    assert_eq!(scope.secret_name(Purpose::Peer), "lumen-peer-tls");
}
