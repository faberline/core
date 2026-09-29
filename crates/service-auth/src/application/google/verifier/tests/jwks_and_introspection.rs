use super::*;

// -- AC3: kid miss refetches exactly once per window --------------------

fn token_with_unknown_kid() -> String {
    mint_with_kid(
        "rotated-key-2",
        serde_json::json!({
            "iss": "https://accounts.google.com", "aud": AUDIENCE,
            "email": IDENTITY, "email_verified": true,
            "exp": now_unix() + 3600,
        }),
    )
}

fn unknown_kid_error() -> GoogleAuthError {
    GoogleAuthError::UnknownSigningKey {
        kid: "rotated-key-2".to_string(),
    }
}

#[tokio::test]
async fn unknown_kid_refetches_once_then_is_rate_limited() {
    let jwks = CountingJwks::serving();
    let clock = FakeClock::new(1_700_000_000);
    let verifier = verifier_with(jwks.clone(), None, clock.clone());
    let rotated = token_with_unknown_kid();

    assert_eq!(
        verifier.verify_id_token(&rotated).await.unwrap_err(),
        unknown_kid_error()
    );
    assert_eq!(jwks.calls(), 1, "a kid miss triggers exactly one fetch");

    assert_eq!(
        verifier.verify_id_token(&rotated).await.unwrap_err(),
        unknown_kid_error()
    );
    assert_eq!(
        jwks.calls(),
        1,
        "a second miss inside the window must not reach Google — this is \
             what keeps fabricated kid values from being amplified upstream"
    );

    clock.advance(DEFAULT_JWKS_REFETCH_MIN_INTERVAL.as_secs() + 1);
    let _ = verifier.verify_id_token(&rotated).await;
    assert_eq!(jwks.calls(), 2, "the window reopens after the interval");
}

#[tokio::test]
async fn a_key_set_fetched_inside_the_window_is_treated_as_authoritative() {
    let jwks = CountingJwks::serving();
    let clock = FakeClock::new(1_700_000_000);
    let verifier = verifier_with(jwks.clone(), None, clock.clone());

    verifier.verify_id_token(&id_token()).await.unwrap();
    assert_eq!(jwks.calls(), 1);

    // A kid missing from a key set fetched moments ago is missing because
    // Google does not publish it, not because the cache is stale.
    // Refetching would ask the same question and get the same answer.
    assert_eq!(
        verifier
            .verify_id_token(&token_with_unknown_kid())
            .await
            .unwrap_err(),
        unknown_kid_error()
    );
    assert_eq!(jwks.calls(), 1);

    // The cost of that rule, pinned so it stays a decision rather than a
    // surprise: a genuine rotation is invisible until the window reopens.
    clock.advance(DEFAULT_JWKS_REFETCH_MIN_INTERVAL.as_secs() + 1);
    let _ = verifier.verify_id_token(&token_with_unknown_kid()).await;
    assert_eq!(jwks.calls(), 2);
}

#[tokio::test]
async fn unreachable_jwks_is_an_upstream_failure_not_a_rejection() {
    let verifier = verifier_with(
        CountingJwks::unavailable(),
        None,
        FakeClock::new(1_700_000_000),
    );
    let error = verifier.verify_id_token(&id_token()).await.unwrap_err();
    assert!(matches!(error, GoogleAuthError::SigningKeyUnavailable(_)));
    assert!(error.is_upstream_failure());
    assert!(matches!(AuthError::from(error), AuthError::Unavailable(_)));
}

// -- AC4: introspection is cached --------------------------------------

#[tokio::test]
async fn introspection_result_is_cached_for_the_bounded_ttl() {
    let introspection = CountingIntrospection::live(IDENTITY, 3147);
    let clock = FakeClock::new(1_700_000_000);
    let verifier = verifier_with(
        CountingJwks::serving(),
        Some(introspection.clone()),
        clock.clone(),
    );

    for _ in 0..4 {
        let email = verifier
            .introspect_access_token("ya29.opaque")
            .await
            .unwrap();
        assert_eq!(email, IDENTITY);
    }
    assert_eq!(
        introspection.calls(),
        1,
        "repeats inside the TTL must not re-ask Google"
    );

    // expires_in is 3147s but the ceiling is 300s, so the ceiling wins.
    clock.advance(DEFAULT_INTROSPECTION_TTL_CEILING.as_secs() + 1);
    verifier
        .introspect_access_token("ya29.opaque")
        .await
        .unwrap();
    assert_eq!(
        introspection.calls(),
        2,
        "the ceiling, not expires_in, bounds how long a revoked credential works"
    );
}

// -- AC5: unreachable is distinct from rejected ------------------------

#[tokio::test]
async fn unreachable_introspection_is_distinct_from_a_rejected_credential() {
    let unreachable = verifier_with(
        CountingJwks::serving(),
        Some(CountingIntrospection::unreachable()),
        FakeClock::new(1_700_000_000),
    );
    let outage = unreachable
        .introspect_access_token("ya29.opaque")
        .await
        .unwrap_err();

    let rejecting = verifier_with(
        CountingJwks::serving(),
        Some(CountingIntrospection::invalid()),
        FakeClock::new(1_700_000_000),
    );
    let rejected = rejecting
        .introspect_access_token("ya29.opaque")
        .await
        .unwrap_err();

    assert!(matches!(
        outage,
        GoogleAuthError::IntrospectionUnavailable(_)
    ));
    assert_eq!(rejected, GoogleAuthError::Rejected);
    assert_ne!(outage, rejected);

    // And they must not collapse into the same thing on the wire either.
    assert!(matches!(AuthError::from(outage), AuthError::Unavailable(_)));
    assert!(matches!(
        AuthError::from(rejected),
        AuthError::Unauthenticated
    ));
}

// -- AC6: authentication is not authorization --------------------------

#[tokio::test]
async fn verified_identity_absent_from_the_registry_is_rejected() {
    let verifier = verifier_with(
        CountingJwks::serving(),
        Some(CountingIntrospection::live(
            "someone-else@axiom-502607.iam.gserviceaccount.com",
            3600,
        )),
        FakeClock::new(1_700_000_000),
    );
    let error = verifier
        .authenticate_credential("ya29.stranger")
        .await
        .unwrap_err();
    assert_eq!(error, GoogleAuthError::NotInRegistry);
    assert!(!error.is_upstream_failure());
}
