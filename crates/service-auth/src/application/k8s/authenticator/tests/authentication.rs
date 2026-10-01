use super::*;

#[tokio::test]
async fn a_token_minted_for_this_service_authenticates() {
    let backend = Arc::new(ScriptedBackend::default().with_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    ))));
    let auth = authenticator(backend.clone(), Arc::new(ManualClock::new(0)));

    let principal = auth.authenticate("opaque-token").await.unwrap();
    assert_eq!(principal.namespace(), "tenant-a");
    assert_eq!(principal.name(), "reader");
    assert_eq!(
        backend.token_calls.lock().unwrap()[0],
        vec![AUDIENCE.to_string()],
        "the audience must be requested explicitly, never left to the apiserver default"
    );
}

/// The unsafe-looking empty audience request is reachable only through
/// the constructor that names why it exists. TokenReview, not this
/// library, then validates the token against the apiserver's audiences.
#[tokio::test]
async fn the_explicit_kubernetes_default_profile_accepts_a_default_ksa_token() {
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Ok(reviewed("system:serviceaccount:tenant-a:reader", &[]))),
    );
    let config = DelegatedAuthConfig::kubernetes_default();
    assert!(config.uses_kubernetes_default());
    let auth =
        DelegatedAuthenticator::with_clock(backend.clone(), config, Arc::new(ManualClock::new(0)));

    let principal = auth.authenticate("default-ksa-token").await.unwrap();
    assert_eq!(principal.namespace(), "tenant-a");
    assert_eq!(principal.name(), "reader");
    assert_eq!(
        backend.token_calls.lock().unwrap()[0],
        Vec::<String>::new(),
        "the explicit Kubernetes-default profile must omit requested audiences"
    );
}

#[tokio::test]
async fn the_kubernetes_default_profile_still_rejects_bad_identity_shapes() {
    for (username, authenticated, reason) in [
        ("alice@example.com", true, "not_a_service_account"),
        (
            "system:serviceaccount:tenant-a:reader",
            false,
            "not_authenticated",
        ),
    ] {
        let mut outcome = reviewed(username, &[]);
        outcome.authenticated = authenticated;
        let backend = Arc::new(ScriptedBackend::default().with_token(Ok(outcome)));
        let auth = DelegatedAuthenticator::with_clock(
            backend,
            DelegatedAuthConfig::kubernetes_default(),
            Arc::new(ManualClock::new(0)),
        );
        let error = auth.authenticate("default-ksa-token").await.unwrap_err();
        assert_eq!(error.reason(), reason);
    }
}

/// AC1: a token the apiserver considers valid, but not for us.
#[tokio::test]
async fn a_token_for_another_audience_is_rejected_even_though_it_is_valid() {
    for granted in [
        vec!["https://kubernetes.default.svc"],
        vec!["some.other.service"],
        vec![],
    ] {
        let backend = Arc::new(ScriptedBackend::default().with_token(Ok(reviewed(
            "system:serviceaccount:tenant-a:reader",
            &granted,
        ))));
        let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

        let error = auth.authenticate("opaque-token").await.unwrap_err();
        assert_eq!(
            error.reason(),
            "audience_mismatch",
            "audiences {granted:?} must not satisfy {AUDIENCE}"
        );
    }
}

#[tokio::test]
async fn an_unauthenticated_review_is_a_401_not_a_503() {
    let backend = Arc::new(
        ScriptedBackend::default().with_token(Ok(TokenReviewOutcome {
            authenticated: false,
            error: Some("token expired".into()),
            ..Default::default()
        })),
    );
    let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

    let error = auth.authenticate("expired-token").await.unwrap_err();
    assert_eq!(error.reason(), "not_authenticated");
    assert!(matches!(error, DelegatedAuthError::Unauthenticated(_)));
}

/// AC2: `authenticated: true` is not enough. This is the rejection that
/// keeps a delegating service from becoming a second identity provider.
#[tokio::test]
async fn a_verified_non_service_account_is_rejected_before_any_authorization() {
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Ok(reviewed("alice@example.com", &[AUDIENCE])))
            .with_access(Ok(AccessReviewOutcome::allow())),
    );
    let auth = authenticator(backend.clone(), Arc::new(ManualClock::new(0)));

    let error = auth.authenticate("google-token").await.unwrap_err();
    assert_eq!(error.reason(), "not_a_service_account");
    assert_eq!(
        backend.access_call_count(),
        0,
        "authorization must never be reached for a rejected identity"
    );
}

/// R4: the authorizer is entitled to everything the authenticator learned.
#[tokio::test]
async fn the_whole_reviewed_identity_reaches_the_access_review() {
    let mut outcome = reviewed("system:serviceaccount:tenant-a:reader", &[AUDIENCE]);
    outcome.identity.groups = vec![
        "system:serviceaccounts".into(),
        "system:serviceaccounts:tenant-a".into(),
    ];
    outcome.identity.extra = BTreeMap::from([(
        "authentication.kubernetes.io/pod-name".to_string(),
        vec!["client-0".to_string()],
    )]);
    let expected = outcome.identity.clone();
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Ok(outcome))
            .with_access(Ok(AccessReviewOutcome::allow())),
    );
    let auth = authenticator(backend.clone(), Arc::new(ManualClock::new(0)));

    let principal = auth.authenticate("opaque-token").await.unwrap();
    auth.authorize(&principal, &attributes()).await.unwrap();

    let (identity, sent) = backend.access_calls.lock().unwrap()[0].clone();
    assert_eq!(identity, expected);
    assert_eq!(sent, attributes());
}

#[tokio::test]
async fn a_denied_operation_is_a_403_naming_the_operation_not_the_caller() {
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Ok(reviewed(
                "system:serviceaccount:tenant-a:reader",
                &[AUDIENCE],
            )))
            .with_access(Ok(AccessReviewOutcome::deny("no matching rule"))),
    );
    let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

    let principal = auth.authenticate("opaque-token").await.unwrap();
    let error = auth.authorize(&principal, &attributes()).await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Denied(_)));
    assert_eq!(
        error.to_string(),
        "not permitted to get example.test/widgets/blue in serving"
    );
}
