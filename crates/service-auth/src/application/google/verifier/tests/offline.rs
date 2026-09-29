use super::*;

// -- Construction refuses the one configuration that fails open --------

/// `jsonwebtoken` reads an empty `set_audience` as "do not check the
/// audience". A verifier built that way accepts any ID token Google ever
/// minted, for any service, and reports nothing — so the check has to
/// happen where the mistake is still visible: at construction.
#[test]
fn a_verifier_with_no_audience_is_refused_at_construction() {
    let err = GoogleVerifier::with_sources(
        true,
        registry(),
        GoogleAuthConfig::new(Vec::<String>::new()),
        CountingJwks::serving(),
        None,
        FakeClock::new(1_700_000_000),
    )
    .unwrap_err();
    assert!(err.to_string().contains("audience"), "{err}");

    // A list of blanks is the same mistake wearing a costume: an unset
    // env var read straight into the config produces exactly this.
    assert!(GoogleVerifier::with_sources(
        true,
        registry(),
        GoogleAuthConfig::new(["", "  "]),
        CountingJwks::serving(),
        None,
        FakeClock::new(1_700_000_000),
    )
    .is_err());
}

// -- AC1: the positive offline path -----------------------------------

#[tokio::test]
async fn real_shaped_id_token_verifies_offline_against_the_published_jwks() {
    let verifier = offline_verifier();
    let email = verifier.verify_id_token(&id_token()).await.unwrap();
    assert_eq!(email, IDENTITY);
}

#[tokio::test]
async fn steady_state_verification_makes_no_network_call() {
    let jwks = CountingJwks::serving();
    let verifier = verifier_with(jwks.clone(), None, FakeClock::new(1_700_000_000));
    let token = id_token();

    verifier.verify_id_token(&token).await.unwrap();
    assert_eq!(jwks.calls(), 1, "the first verification primes the cache");
    for _ in 0..5 {
        verifier.verify_id_token(&token).await.unwrap();
    }
    assert_eq!(jwks.calls(), 1, "a warm cache must not touch the network");
}

// -- AC8 (from the spike): the identity drives the UNMODIFIED role map --

#[tokio::test]
async fn verified_identity_drives_the_unmodified_role_map() {
    let verifier = offline_verifier();
    let principal = verifier.authenticate_credential(&id_token()).await.unwrap();

    assert!(
        principal.ensure("products", Role::Read).is_ok(),
        "granted collection at the granted role -> allowed"
    );
    assert!(
        principal.ensure("products", Role::Admin).is_err(),
        "granted collection above the granted role -> denied"
    );
    assert!(
        principal.ensure("secrets", Role::Read).is_err(),
        "ungranted collection -> denied"
    );
    assert_eq!(
        principal.subject(),
        Some("dev:lumen-dev"),
        "audit subject survives the hop"
    );
}

// -- AC2: each negative rejected, and distinguishable -------------------

#[tokio::test]
async fn token_minted_for_another_audience_is_rejected_as_audience() {
    let verifier = offline_verifier();
    let token = mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://accounts.google.com",
            "aud": "some-other-service",
            "email": IDENTITY, "email_verified": true,
            "exp": now_unix() + 3600,
        }),
    );
    assert_eq!(
        verifier.verify_id_token(&token).await.unwrap_err(),
        GoogleAuthError::Invalid(InvalidReason::Audience)
    );
}

#[tokio::test]
async fn token_from_an_untrusted_issuer_is_rejected_as_issuer() {
    let verifier = offline_verifier();
    let token = mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://evil.example.com",
            "aud": AUDIENCE,
            "email": IDENTITY, "email_verified": true,
            "exp": now_unix() + 3600,
        }),
    );
    assert_eq!(
        verifier.verify_id_token(&token).await.unwrap_err(),
        GoogleAuthError::Invalid(InvalidReason::Issuer)
    );
}

#[tokio::test]
async fn tampered_signature_is_rejected_as_signature() {
    let verifier = offline_verifier();
    let token = id_token();
    // Flip one character of the signature; everything else stays
    // byte-identical.
    let (body, signature) = token.rsplit_once('.').unwrap();
    let first = signature.chars().next().unwrap();
    let flipped = if first == 'A' { 'B' } else { 'A' };
    let tampered = format!("{body}.{flipped}{}", &signature[1..]);

    assert_eq!(
        verifier.verify_id_token(&tampered).await.unwrap_err(),
        GoogleAuthError::Invalid(InvalidReason::Signature)
    );
}

#[tokio::test]
async fn expired_token_is_rejected_as_expired() {
    let verifier = offline_verifier();
    let token = mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://accounts.google.com",
            "aud": AUDIENCE,
            "email": IDENTITY, "email_verified": true,
            "exp": now_unix() - 3600,
        }),
    );
    assert_eq!(
        verifier.verify_id_token(&token).await.unwrap_err(),
        GoogleAuthError::Invalid(InvalidReason::Expired)
    );
}

#[tokio::test]
async fn unverified_email_is_rejected_distinctly_from_a_missing_one() {
    let verifier = offline_verifier();
    let unverified = mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://accounts.google.com", "aud": AUDIENCE,
            "email": IDENTITY, "email_verified": false,
            "exp": now_unix() + 3600,
        }),
    );
    assert_eq!(
        verifier.verify_id_token(&unverified).await.unwrap_err(),
        GoogleAuthError::EmailUnverified
    );

    let no_email = mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://accounts.google.com", "aud": AUDIENCE,
            "exp": now_unix() + 3600,
        }),
    );
    assert_eq!(
        verifier.verify_id_token(&no_email).await.unwrap_err(),
        GoogleAuthError::EmailMissing
    );
}
