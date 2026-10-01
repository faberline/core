use super::*;

#[test]
fn role_covers() {
    assert!(Role::Admin.covers(Role::Read));
    assert!(Role::Admin.covers(Role::Admin));
    assert!(!Role::Read.covers(Role::Admin));
    assert!(Role::Write.covers(Role::Read));
}

#[test]
fn role_compare_total_order() {
    let mut roles = vec![Role::Admin, Role::Read, Role::Write];
    roles.sort();
    assert_eq!(roles, vec![Role::Read, Role::Write, Role::Admin]);
}

#[test]
fn open_principal_allows_everything() {
    assert!(RoleMapPrincipal::Open.ensure("any", Role::Admin).is_ok());
    assert_eq!(RoleMapPrincipal::Open.subject(), None);
}

#[test]
fn per_resource_role_enforced() {
    let p = RoleMapPrincipal::Token(token(&[("users", Role::Read)]));
    assert!(p.ensure("users", Role::Read).is_ok());
    assert!(p.ensure("users", Role::Write).is_err());
    assert!(p.ensure("other", Role::Read).is_err());
}

#[test]
fn wildcard_resource_covers_all() {
    let p = RoleMapPrincipal::Token(token(&[("*", Role::Write)]));
    assert!(p.ensure("any", Role::Read).is_ok());
    assert!(p.ensure("any", Role::Write).is_ok());
    assert!(p.ensure("any", Role::Admin).is_err());
}

#[test]
fn specific_resource_role_overrides_no_wildcard() {
    // Per-resource role without a wildcard grant — only that resource.
    let p = RoleMapPrincipal::Token(token(&[("users", Role::Admin)]));
    assert!(p.ensure("users", Role::Admin).is_ok());
    assert!(p.ensure("other", Role::Read).is_err());
}

#[test]
fn ensure_denied_carries_structured_fields() {
    let p = RoleMapPrincipal::Token(token(&[("users", Role::Read)]));
    let denied = p.ensure("users", Role::Admin).unwrap_err();
    assert_eq!(denied.subject, "tester");
    assert_eq!(denied.needed, Role::Admin);
    assert_eq!(denied.resource, "users");
}

#[test]
fn principal_subject_returns_some_for_token_and_none_for_open() {
    let p = RoleMapPrincipal::Token(token(&[("u", Role::Read)]));
    assert_eq!(p.subject(), Some("tester"));
    assert_eq!(RoleMapPrincipal::Open.subject(), None);
}

#[test]
fn static_role_map_verifier_open_mode_without_bearer_returns_open_principal() {
    let verifier = StaticRoleMapVerifier::open();
    let p = verifier.authenticate(&HeaderMap::new()).unwrap();
    assert!(matches!(p, RoleMapPrincipal::Open));
}

#[test]
fn static_role_map_verifier_known_bearer_returns_token_principal() {
    let verifier = StaticRoleMapVerifier::new(
        true,
        HashMap::from([("abc".to_string(), token(&[("u", Role::Write)]))]),
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        "Bearer abc".parse().unwrap(),
    );
    let p = verifier.authenticate(&headers).unwrap();
    assert_eq!(p.subject(), Some("tester"));
    assert!(p.ensure("u", Role::Write).is_ok());
}

#[test]
fn static_role_map_verifier_invalid_bearer_rejects_unauthenticated() {
    let verifier = StaticRoleMapVerifier::new(
        true,
        HashMap::from([("abc".to_string(), token(&[("u", Role::Read)]))]),
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        "Bearer nope".parse().unwrap(),
    );
    let err = verifier.authenticate(&headers).unwrap_err();
    assert!(matches!(err, AuthError::Unauthenticated));
}

#[test]
fn static_role_map_verifier_required_missing_bearer_rejects_unauthenticated() {
    let verifier = StaticRoleMapVerifier::new(true, HashMap::new());
    let err = verifier.authenticate(&HeaderMap::new()).unwrap_err();
    assert!(matches!(err, AuthError::Unauthenticated));
}
