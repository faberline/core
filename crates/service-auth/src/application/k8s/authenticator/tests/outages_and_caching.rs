use super::*;

/// AC7: a transport failure with nothing cached fails closed as 503 —
/// distinct from both 401 and 403, so an operator can tell the difference.
#[tokio::test]
async fn an_outage_with_no_cached_answer_fails_closed() {
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Err(ReviewError::Transport("connection refused".into()))),
    );
    let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

    let error = auth.authenticate("opaque-token").await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Unavailable(_)));
    assert_eq!(error.reason(), "transport");
}

#[tokio::test]
async fn a_missing_delegation_grant_fails_closed_and_says_so() {
    let backend = Arc::new(
        ScriptedBackend::default().with_token(Err(ReviewError::NotDelegated(
            "serviceaccount cannot create tokenreviews".into(),
        ))),
    );
    let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

    let error = auth.authenticate("opaque-token").await.unwrap_err();
    assert_eq!(error.reason(), "not_delegated");
}

/// AC7: an authorizer that partially failed has not said "no", but it has
/// certainly not said "yes".
#[tokio::test]
async fn a_partial_authorizer_failure_is_not_an_allow() {
    let backend = Arc::new(
        ScriptedBackend::default()
            .with_token(Ok(reviewed(
                "system:serviceaccount:tenant-a:reader",
                &[AUDIENCE],
            )))
            .with_access(Ok(AccessReviewOutcome {
                allowed: true,
                denied: false,
                reason: None,
                evaluation_error: Some("webhook authorizer unreachable".into()),
            })),
    );
    let auth = authenticator(backend, Arc::new(ManualClock::new(0)));

    let principal = auth.authenticate("opaque-token").await.unwrap();
    let error = auth.authorize(&principal, &attributes()).await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Unavailable(_)));
    assert_eq!(error.reason(), "malformed_response");
}

/// A second request for the same token asks the apiserver nothing.
#[tokio::test]
async fn a_repeated_request_is_answered_from_cache() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend.clone(), clock);

    for _ in 0..5 {
        let principal = auth.authenticate("opaque-token").await.unwrap();
        auth.authorize(&principal, &attributes()).await.unwrap();
    }
    assert_eq!(backend.token_call_count(), 1);
    assert_eq!(backend.access_call_count(), 1);
    assert_eq!(auth.metrics().token_cache_hits.get(), 4);
    assert_eq!(auth.metrics().access_cache_hits.get(), 4);
}

/// AC6: a revoked allow stops working, on a schedule, with the apiserver
/// answering normally the whole time.
#[tokio::test]
async fn a_revoked_allow_expires_within_the_documented_bound() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend.clone(), clock.clone());

    let principal = auth.authenticate("opaque-token").await.unwrap();
    auth.authorize(&principal, &attributes()).await.unwrap();

    // The RoleBinding is removed in Kubernetes.
    backend.set_access(Ok(AccessReviewOutcome::deny("no matching rule")));
    auth.authorize(&principal, &attributes())
        .await
        .expect("inside the TTL the stale allow is still served");

    clock.advance(Duration::from_secs(301));
    let error = auth.authorize(&principal, &attributes()).await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Denied(_)));
    assert_eq!(auth.revocation_bound(), Duration::from_secs(360));
}

/// AC6: the outage path is bounded. Inside the window the last known
/// answer is served; past it there is no path back to it.
#[tokio::test]
async fn an_outage_serves_a_stale_allow_only_inside_the_window() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend.clone(), clock.clone());

    let principal = auth.authenticate("opaque-token").await.unwrap();
    auth.authorize(&principal, &attributes()).await.unwrap();

    backend.set_access(Err(ReviewError::Transport("connection refused".into())));
    clock.advance(Duration::from_secs(301));
    auth.authorize(&principal, &attributes())
        .await
        .expect("30s past expiry is inside the 60s stale window");
    assert_eq!(auth.metrics().access_cache_stale.get(), 1);

    clock.advance(Duration::from_secs(60));
    let error = auth.authorize(&principal, &attributes()).await.unwrap_err();
    assert!(
        matches!(error, DelegatedAuthError::Unavailable(_)),
        "past the stale window an outage must fail closed, not keep serving"
    );
}

/// The same bound applies to authentication: an outage cannot turn a
/// short-lived token into an indefinite one.
#[tokio::test]
async fn an_outage_cannot_extend_an_authentication_indefinitely() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    let auth = authenticator(backend.clone(), clock.clone());
    auth.authenticate("opaque-token").await.unwrap();

    backend.set_token(Err(ReviewError::Transport("connection refused".into())));
    clock.advance(Duration::from_secs(301));
    auth.authenticate("opaque-token")
        .await
        .expect("inside the stale window");

    clock.advance(Duration::from_secs(60));
    let error = auth.authenticate("opaque-token").await.unwrap_err();
    assert!(matches!(error, DelegatedAuthError::Unavailable(_)));
    assert_eq!(auth.metrics().token_cache_stale.get(), 1);
}

/// A rejection is cached too, so a flood of bad tokens is not a way to
/// generate apiserver load — but only for the short deny TTL.
#[tokio::test]
async fn a_rejection_is_cached_briefly_and_then_re_reviewed() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed("alice@example.com", &[AUDIENCE])));
    let auth = authenticator(backend.clone(), clock.clone());

    for _ in 0..3 {
        assert!(auth.authenticate("google-token").await.is_err());
    }
    assert_eq!(backend.token_call_count(), 1);

    clock.advance(Duration::from_secs(31));
    assert!(auth.authenticate("google-token").await.is_err());
    assert_eq!(backend.token_call_count(), 2);
}

/// Two callers whose usernames match but whose groups differ are two
/// different authorization questions.
#[tokio::test]
async fn the_decision_cache_key_covers_the_whole_identity_not_just_the_username() {
    let (backend, clock) = fixture();
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend.clone(), clock);

    let mut first = reviewed_identity("system:serviceaccount:tenant-a:reader");
    first.groups = vec!["group-one".into()];
    let mut second = first.clone();
    second.groups = vec!["group-two".into()];

    let a = ServiceAccountPrincipal::from_review(true, first).unwrap();
    let b = ServiceAccountPrincipal::from_review(true, second).unwrap();
    auth.authorize(&a, &attributes()).await.unwrap();
    auth.authorize(&b, &attributes()).await.unwrap();

    assert_eq!(
        backend.access_call_count(),
        2,
        "a differing group list must not be answered from the other caller's entry"
    );
}

/// Different resources are different questions, even for one caller.
#[tokio::test]
async fn each_resource_and_verb_is_its_own_cached_decision() {
    let (backend, clock) = fixture();
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend.clone(), clock);
    let principal = ServiceAccountPrincipal::from_review(
        true,
        reviewed_identity("system:serviceaccount:tenant-a:reader"),
    )
    .unwrap();

    let read = attributes();
    let mut write = attributes();
    write.verb = "update".into();
    let mut other = attributes();
    other.name = Some("green".into());

    for check in [&read, &write, &other] {
        auth.authorize(&principal, check).await.unwrap();
    }
    assert_eq!(backend.access_call_count(), 3);
}
