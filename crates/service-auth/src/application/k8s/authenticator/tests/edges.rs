use super::*;

/// A configuration with no audience is refused at construction, because
/// there is no safe way to run without one.
#[test]
fn a_configuration_without_an_audience_is_refused() {
    assert_eq!(DelegatedAuthConfig::new(vec![]), Err(MissingAudience));
    assert_eq!(
        DelegatedAuthConfig::new(vec![String::new()]),
        Err(MissingAudience)
    );
    assert!(DelegatedAuthConfig::new(vec![AUDIENCE.into()]).is_ok());
    assert!(DelegatedAuthConfig::kubernetes_default().uses_kubernetes_default());
    assert!(DelegatedAuthConfig::kubernetes_default()
        .audiences()
        .is_empty());
}

#[tokio::test]
async fn an_empty_credential_never_reaches_the_apiserver() {
    let (backend, clock) = fixture();
    let auth = authenticator(backend.clone(), clock);
    let error = auth.authenticate("").await.unwrap_err();
    assert_eq!(error.reason(), "missing_credential");
    assert_eq!(backend.token_call_count(), 0);
}

/// R10 / AC7: no error rendering, and no metric, may carry the token.
#[tokio::test]
async fn no_rendered_error_or_metric_contains_the_token() {
    let secret = "super-secret-bearer-token";
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed("alice@example.com", &[AUDIENCE])));
    let auth = authenticator(backend.clone(), clock);

    let error = auth.authenticate(secret).await.unwrap_err();
    assert!(!error.to_string().contains(secret));
    assert!(!format!("{error:?}").contains(secret));
    assert!(!auth.metrics().render().contains(secret));
    assert!(!fingerprint(secret).contains(secret));
    assert_eq!(fingerprint(secret).len(), 12);
    assert_eq!(
        fingerprint(secret),
        fingerprint(secret),
        "a fingerprint must be stable to be useful for correlation"
    );
    assert_ne!(fingerprint(secret), fingerprint("another-token"));
}

#[tokio::test]
async fn the_rendered_metrics_declare_every_counter() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    backend.set_access(Ok(AccessReviewOutcome::allow()));
    let auth = authenticator(backend, clock);
    let principal = auth.authenticate("opaque-token").await.unwrap();
    auth.authorize(&principal, &attributes()).await.unwrap();

    let rendered = auth.metrics().render();
    for name in [
        "delegated_auth_token_reviews_total",
        "delegated_auth_token_cache_misses_total",
        "delegated_auth_authenticated_total",
        "delegated_auth_access_reviews_total",
        "delegated_auth_allowed_total",
        "delegated_auth_denied_total",
        "delegated_auth_unavailable_total",
    ] {
        assert!(rendered.contains(name), "{name} is missing from the scrape");
    }
    assert!(rendered.contains("delegated_auth_allowed_total 1"));
}

/// The 503 arm must not be reachable by a caller-shaped input; it exists
/// only for the apiserver being unreachable.
#[test]
fn the_http_mapping_keeps_the_three_outcomes_distinct() {
    let unauthenticated: AuthError =
        DelegatedAuthError::Unauthenticated(AuthRejection::AudienceMismatch).into();
    assert!(matches!(unauthenticated, AuthError::Unauthenticated));

    let denied: AuthError = DelegatedAuthError::Denied(attributes()).into();
    match denied {
        AuthError::Forbidden(message) => {
            assert!(message.contains("get example.test/widgets/blue in serving"))
        }
        other => panic!("a deny must be 403, got {other:?}"),
    }

    let unavailable: AuthError =
        DelegatedAuthError::Unavailable(ReviewError::Transport("down".into())).into();
    assert!(matches!(unavailable, AuthError::Unavailable(_)));
}

#[tokio::test]
async fn invalidating_the_cache_forces_a_fresh_review() {
    let (backend, clock) = fixture();
    backend.set_token(Ok(reviewed(
        "system:serviceaccount:tenant-a:reader",
        &[AUDIENCE],
    )));
    let auth = authenticator(backend.clone(), clock);

    auth.authenticate("opaque-token").await.unwrap();
    auth.invalidate();
    auth.authenticate("opaque-token").await.unwrap();
    assert_eq!(backend.token_call_count(), 2);
}
