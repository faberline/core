use super::*;

// -- R5: discrimination by shape, not by trying each in turn ------------

#[test]
fn credentials_are_classified_by_shape() {
    assert_eq!(classify(&id_token()), Credential::GoogleIdToken);
    assert_eq!(classify("ya29.a0AfB_opaque"), Credential::Opaque);
    assert_eq!(classify("plain-preshared-secret"), Credential::Opaque);
    assert_eq!(classify("not.a.jwt"), Credential::Opaque);
    // Three segments but no kid: Google always sets one, so this is not a
    // Google ID token and must not be sent down the offline path.
    let mut header = Header::new(Algorithm::RS256);
    header.kid = None;
    let kidless = encode(
        &header,
        &serde_json::json!({"sub": "x", "exp": now_unix() + 60}),
        &EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).unwrap(),
    )
    .unwrap();
    assert_eq!(classify(&kidless), Credential::Opaque);
}

#[tokio::test]
async fn a_preshared_secret_resolves_from_the_registry_without_asking_google() {
    let introspection = CountingIntrospection::live(IDENTITY, 3600);
    let registry = Arc::new(ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([(
            "preshared-secret".to_string(),
            TokenClaims {
                subject: "svc:legacy".to_string(),
                roles: HashMap::from([("products".to_string(), Role::Write)]),
            },
        )]),
    ));
    let verifier = GoogleVerifier::with_sources(
        true,
        registry,
        GoogleAuthConfig::new([AUDIENCE]),
        CountingJwks::serving(),
        Some(introspection.clone()),
        FakeClock::new(1_700_000_000),
    )
    .expect("one audience is configured");

    let principal = verifier
        .authenticate_credential("preshared-secret")
        .await
        .unwrap();
    assert_eq!(principal.subject(), Some("svc:legacy"));
    assert_eq!(
        introspection.calls(),
        0,
        "an existing bearer secret must not acquire a Google round trip"
    );
}

// -- #2678 AC1/AC2: the two namespaces are disjoint ---------------------

/// AC1. Once the identity is verified, it is judged by the same role map
/// as a secret-keyed entry — same claims in, same decision out. The
/// namespace decides *how you prove who you are*, never *what you may do*.
#[tokio::test]
async fn an_identity_keyed_entry_authorizes_identically_to_a_secret_keyed_one() {
    let by_identity = verifier_with(CountingJwks::serving(), None, FakeClock::new(1_700_000_000));
    let by_secret = GoogleVerifier::with_sources(
        true,
        Arc::new(ReloadableRoleMapVerifier::with_registry(
            true,
            Registry::from_tokens(HashMap::from([(
                "preshared-secret".to_string(),
                dev_claims(),
            )])),
        )),
        GoogleAuthConfig::new([AUDIENCE]),
        CountingJwks::serving(),
        None,
        FakeClock::new(1_700_000_000),
    )
    .expect("one audience is configured");

    let from_identity = by_identity
        .authenticate_credential(&id_token())
        .await
        .unwrap();
    let from_secret = by_secret
        .authenticate_credential("preshared-secret")
        .await
        .unwrap();

    assert_eq!(from_identity.subject(), from_secret.subject());
    for (resource, role) in [
        ("products", Role::Read),
        ("products", Role::Write),
        ("orders", Role::Read),
    ] {
        assert_eq!(
            from_identity.ensure(resource, role).is_ok(),
            from_secret.ensure(resource, role).is_ok(),
            "{resource}/{role:?} must decide the same either way"
        );
    }
}

/// AC2. A bearer secret whose text happens to be a valid email address
/// must not match an email-keyed entry. Presenting a string is not proving
/// an identity; if one namespace served both, anyone who learned an
/// authorized email would hold a working credential.
#[tokio::test]
async fn a_bearer_secret_spelled_like_an_email_does_not_match_an_identity_entry() {
    // No introspection source: reaching Google is itself the proof that
    // the string was not resolved locally as a secret.
    let verifier = verifier_with(CountingJwks::serving(), None, FakeClock::new(1_700_000_000));

    let error = verifier
        .authenticate_credential(IDENTITY)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        GoogleAuthError::IntrospectionNotConfigured,
        "the email text must fall through to introspection, not resolve as a secret"
    );
}

#[tokio::test]
async fn opaque_credential_with_no_introspector_configured_says_so() {
    let verifier = verifier_with(CountingJwks::serving(), None, FakeClock::new(1_700_000_000));
    assert_eq!(
        verifier
            .authenticate_credential("ya29.opaque")
            .await
            .unwrap_err(),
        GoogleAuthError::IntrospectionNotConfigured
    );
}

// -- the middleware-facing surface -------------------------------------

#[tokio::test]
async fn authenticate_async_accepts_a_bearer_id_token() {
    let verifier = offline_verifier();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", id_token()).parse().unwrap(),
    );
    let principal = verifier.authenticate_async(&headers).await.unwrap();
    assert_eq!(principal.subject(), Some("dev:lumen-dev"));
}

#[tokio::test]
async fn authenticate_async_without_a_bearer_is_unauthenticated_when_required() {
    let verifier = offline_verifier();
    assert!(matches!(
        verifier.authenticate_async(&HeaderMap::new()).await,
        Err(AuthError::Unauthenticated)
    ));
}

// -- Google's string-typed JSON ----------------------------------------

#[test]
fn tokeninfo_string_typed_fields_are_understood() {
    // Measured against the live endpoint during design: Google sends
    // email_verified and expires_in as strings, not as JSON scalars.
    let parsed: IntrospectedToken =
        serde_json::from_str(r#"{"email":"a@b.com","email_verified":"true","expires_in":"3147"}"#)
            .unwrap();
    assert_eq!(parsed.email.as_deref(), Some("a@b.com"));
    assert!(parsed.email_verified);
    assert_eq!(parsed.expires_in, 3147);

    let native: IntrospectedToken =
        serde_json::from_str(r#"{"email":"a@b.com","email_verified":true,"expires_in":3147}"#)
            .unwrap();
    assert!(native.email_verified);
    assert_eq!(native.expires_in, 3147);
}

#[test]
fn debug_output_never_carries_a_credential() {
    let verifier = offline_verifier();
    let rendered = format!("{verifier:?}");
    assert!(!rendered.contains("ya29"));
    assert!(!rendered.contains("cache"));
}
