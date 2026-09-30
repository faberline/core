use super::*;

fn scope() -> InstanceScope {
    InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
}

fn owner() -> Owner {
    Owner {
        api_version: "lumen.dev/v1".into(),
        kind: "Lumen".into(),
        name: "lumen".into(),
        uid: "0f7d1f4e-0000-4000-8000-000000000000".into(),
    }
}

fn anchor(tag: &str) -> String {
    format!("-----BEGIN CERTIFICATE-----\n{tag}\n-----END CERTIFICATE-----")
}

#[test]
fn a_bundle_round_trips_through_pem_and_its_annotation() {
    let mut bundle = TrustBundle::new();
    bundle.insert(IssuerId::new("pool-a"), anchor("QUFB"));
    bundle.insert(IssuerId::new("pool-b"), anchor("QkJC"));
    let parsed = TrustBundle::parse(&bundle.to_pem(), Some(&bundle.annotation()));
    assert_eq!(parsed, bundle);
    assert_eq!(
        parsed.issuers(),
        vec![IssuerId::new("pool-a"), IssuerId::new("pool-b")]
    );
}

#[test]
fn a_bundle_whose_annotation_disagrees_with_its_contents_is_not_guessed_at() {
    let mut bundle = TrustBundle::new();
    bundle.insert(IssuerId::new("pool-a"), anchor("QUFB"));
    bundle.insert(IssuerId::new("pool-b"), anchor("QkJC"));
    let parsed = TrustBundle::parse(&bundle.to_pem(), Some("pool-a"));
    assert!(
        parsed.is_empty(),
        "pairing two anchors with one id would be a guess about what the fleet trusts"
    );
}

#[test]
fn publishing_trust_writes_no_leaf_keys() {
    let mut bundle = TrustBundle::new();
    bundle.insert(IssuerId::new("pool-a"), anchor("QUFB"));
    let secret = trust_bundle_secret(&scope(), Purpose::Peer, &owner(), &bundle);
    let data = secret["stringData"].as_object().unwrap();
    assert_eq!(data.len(), 1);
    assert!(data.contains_key(TRUST_BUNDLE_KEY));
    assert!(
        !data.contains_key(CERT_KEY) && !data.contains_key(PRIVATE_KEY_KEY),
        "widening trust must not be able to blank the leaf that is serving traffic"
    );
}

#[test]
fn secrets_are_garbage_collected_with_their_instance() {
    let secret = trust_bundle_secret(&scope(), Purpose::Peer, &owner(), &TrustBundle::new());
    let reference = &secret["metadata"]["ownerReferences"][0];
    assert_eq!(reference["controller"], json!(true));
    assert_eq!(reference["blockOwnerDeletion"], json!(true));
    assert_eq!(
        reference["uid"],
        json!("0f7d1f4e-0000-4000-8000-000000000000")
    );
}

#[test]
fn the_secret_lands_in_the_instances_own_namespace() {
    let secret = trust_bundle_secret(&scope(), Purpose::Serving, &owner(), &TrustBundle::new());
    assert_eq!(secret["metadata"]["namespace"], json!("lumen"));
    assert_eq!(secret["metadata"]["name"], json!("lumen-serving-tls"));
}
