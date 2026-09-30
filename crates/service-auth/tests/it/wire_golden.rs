//! Golden pins taken before the DDD P2 changes, through public paths only:
//! the registry and claims documents a service reads, the TokenRequest body
//! and Google introspection answer that cross the wire, and the text of every
//! registry, review and token-request error.

use service_auth::gcp::IntrospectedToken;
use service_auth::k8s::{ReviewError, TokenRequestError, TokenRequestTarget};
use service_auth::{Registry, TokenClaims};

fn chain(err: impl Into<anyhow::Error>) -> (String, String) {
    let err = err.into();
    (err.to_string(), format!("{err:#}"))
}

fn pair(display: &str, alternate: &str) -> (String, String) {
    (display.to_string(), alternate.to_string())
}

#[test]
fn token_claims_decode_is_pinned() {
    for (json, debug) in [
        (
            r#"{"subject":"svc","roles":{"*":"read"}}"#,
            r#"TokenClaims { subject: "svc", roles: {"*": Read} }"#,
        ),
        (
            r#"{"subject":"svc","roles":{"coll-a":"admin"}}"#,
            r#"TokenClaims { subject: "svc", roles: {"coll-a": Admin} }"#,
        ),
        (
            r#"{"subject":"svc"}"#,
            r#"TokenClaims { subject: "svc", roles: {} }"#,
        ),
    ] {
        let claims: TokenClaims = serde_json::from_str(json).unwrap();
        assert_eq!(format!("{claims:?}"), debug, "{json}");
    }
}

#[test]
fn registry_documents_decode_is_pinned() {
    let namespaced = Registry::parse(
        r#"{"tokens":{"s":{"subject":"svc","roles":{"*":"write"}}},"identities":{"a@b.com":{"subject":"a","roles":{"x":"read"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        format!("{namespaced:?}"),
        r#"Registry { tokens: {"s": TokenClaims { subject: "svc", roles: {"*": Write} }}, identities: {"a@b.com": TokenClaims { subject: "a", roles: {"x": Read} }} }"#
    );
    let flat = Registry::parse(r#"{"s":{"subject":"svc","roles":{"*":"write"}}}"#).unwrap();
    assert_eq!(
        format!("{flat:?}"),
        r#"Registry { tokens: {"s": TokenClaims { subject: "svc", roles: {"*": Write} }}, identities: {} }"#
    );
}

#[test]
fn registry_parse_errors_are_pinned() {
    let cases = [
        (
            "not json",
            pair(
                "credential registry must be JSON",
                "credential registry must be JSON: expected ident at line 1 column 2",
            ),
        ),
        (
            "[]",
            pair(
                "credential registry must be a JSON object",
                "credential registry must be a JSON object",
            ),
        ),
        (
            r#"{"s":{"roles":{}}}"#,
            pair(
                "credential registry must map each bearer secret to its claims",
                "credential registry must map each bearer secret to its claims: missing field `subject`",
            ),
        ),
        (
            r#"{"tokens":{"s":1}}"#,
            pair(
                "credential registry `tokens` must map each bearer secret to its claims",
                "credential registry `tokens` must map each bearer secret to its claims: invalid type: integer `1`, expected struct TokenClaims",
            ),
        ),
        (
            r#"{"identities":{"a@b.com":{"subject":"a","roles":{"*":"owner"}}}}"#,
            pair(
                "credential registry `identities` must map each verified identity to its claims",
                "credential registry `identities` must map each verified identity to its claims: unknown variant `owner`, expected one of `read`, `write`, `admin`",
            ),
        ),
    ];
    for (doc, expected) in cases {
        assert_eq!(chain(Registry::parse(doc).unwrap_err()), expected, "{doc}");
    }
}

#[test]
fn registry_merge_errors_are_pinned() {
    let mut into = Registry::parse(
        r#"{"tokens":{"s":{"subject":"svc"}},"identities":{"a@b.com":{"subject":"a"}}}"#,
    )
    .unwrap();
    let other = Registry::parse(r#"{"tokens":{"s":{"subject":"other"}}}"#).unwrap();
    let message = "credential registry sources disagree: `tokens` defines the entry granting `svc` more than once, so there is no way to say which grants are being served";
    assert_eq!(
        chain(into.try_merge(other).unwrap_err()),
        pair(message, message)
    );

    let mut into = Registry::parse(r#"{"identities":{"a@b.com":{"subject":"a"}}}"#).unwrap();
    let other = Registry::parse(r#"{"identities":{"a@b.com":{"subject":"b"}}}"#).unwrap();
    let message = "credential registry sources disagree: `identities` defines `a@b.com` more than once, so there is no way to say which grants are being served";
    assert_eq!(
        chain(into.try_merge(other).unwrap_err()),
        pair(message, message)
    );
}

#[test]
fn review_errors_are_pinned() {
    for (error, display, reason) in [
        (
            ReviewError::Transport("connection refused".into()),
            "review transport failure: connection refused",
            "transport",
        ),
        (
            ReviewError::Malformed("no status".into()),
            "malformed review response: no status",
            "malformed_response",
        ),
        (
            ReviewError::NotDelegated("anonymous".into()),
            "delegated review unavailable: anonymous",
            "not_delegated",
        ),
    ] {
        assert_eq!(error.to_string(), display);
        assert_eq!(error.reason(), reason);
    }
}

#[test]
fn token_request_errors_are_pinned() {
    let forbidden = |username: Option<&str>| TokenRequestError::Forbidden {
        username: username.map(str::to_string),
        namespace: "ns".into(),
        service_account: "sa".into(),
        detail: "denied".into(),
    };
    let cases = [
        (
            TokenRequestError::InvalidTarget {
                field: "namespace",
                value: "a/b".into(),
                reason: "bad".into(),
            },
            "invalid namespace `a/b`: bad",
        ),
        (
            TokenRequestError::NoIdentity {
                detail: "no kubeconfig".into(),
            },
            "no Kubernetes identity to mint a token with: no kubeconfig — this path uses your kubeconfig and nothing else, so `kubectl auth whoami` failing here means the same thing it would there",
        ),
        (
            forbidden(Some("alice")),
            "`alice` may not mint a token for ServiceAccount `ns/sa`: denied. Check with `kubectl auth can-i create serviceaccounts/sa --subresource=token -n ns`; the missing grant is `create` on `serviceaccounts/token` with `resourceNames: [sa]`",
        ),
        (
            forbidden(None),
            "the identity in your kubeconfig may not mint a token for ServiceAccount `ns/sa`: denied. Check with `kubectl auth can-i create serviceaccounts/sa --subresource=token -n ns`; the missing grant is `create` on `serviceaccounts/token` with `resourceNames: [sa]`",
        ),
        (
            TokenRequestError::NoSuchServiceAccount {
                namespace: "ns".into(),
                service_account: "sa".into(),
            },
            "ServiceAccount `ns/sa` does not exist — a token can only be minted for an account that does, and a grant naming one that does not is accepted by RBAC without ever working",
        ),
        (
            TokenRequestError::Transport {
                detail: "reset".into(),
            },
            "the TokenRequest did not complete: reset",
        ),
        (
            TokenRequestError::Malformed {
                detail: "no token".into(),
            },
            "the apiserver accepted the TokenRequest and did not answer it: no token",
        ),
    ];
    for (error, display) in cases {
        assert_eq!(error.to_string(), display);
    }
    assert_eq!(
        TokenRequestError::can_i_command("ns", "sa"),
        "kubectl auth can-i create serviceaccounts/sa --subresource=token -n ns"
    );
}

#[test]
fn token_request_target_errors_are_pinned() {
    let name_rule = "a Kubernetes name is lowercase alphanumerics, `-`, and `.` — nothing else, and in particular no `/`, which would address a different object than the one named";
    for (namespace, service_account, audience, display) in [
        ("a/b", "sa", "x", format!("invalid namespace `a/b`: {name_rule}")),
        ("NS", "sa", "x", format!("invalid namespace `NS`: {name_rule}")),
        (
            "",
            "sa",
            "x",
            "invalid namespace ``: a name is required; this target is never inferred".into(),
        ),
        (
            "ns",
            "..",
            "x",
            "invalid client service account `..`: a Kubernetes name starts and ends with a letter or a digit".into(),
        ),
        (
            "ns",
            "sa",
            " ",
            "invalid audience ` `: an audience-bound token needs an audience; a token minted with none is accepted by every service that does not check, which is the failure this whole path exists to prevent".into(),
        ),
    ] {
        let error = TokenRequestTarget::new(namespace, service_account, audience).unwrap_err();
        assert_eq!(error.to_string(), display);
    }
    let error = TokenRequestTarget::new("ns", "sa", "aud")
        .unwrap()
        .with_expiration_seconds(10)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid expiration `10`: kube-apiserver rejects a TokenRequest below 600 seconds"
    );
}

#[test]
fn token_request_body_is_pinned() {
    let target = TokenRequestTarget::new("ns", "sa", "aud")
        .unwrap()
        .with_expiration_seconds(3600)
        .unwrap();
    assert_eq!(
        target.request_body().to_string(),
        r#"{"apiVersion":"authentication.k8s.io/v1","kind":"TokenRequest","spec":{"audiences":["aud"],"expirationSeconds":3600}}"#
    );
    assert_eq!(
        target.subresource_path(),
        "/api/v1/namespaces/ns/serviceaccounts/sa/token"
    );
    assert_eq!(target.qualified_name(), "ns/sa");
    let default_audience = TokenRequestTarget::kubernetes_default("ns", "sa").unwrap();
    assert_eq!(
        default_audience.request_body().to_string(),
        r#"{"apiVersion":"authentication.k8s.io/v1","kind":"TokenRequest","spec":{"audiences":[],"expirationSeconds":600}}"#
    );
}

#[test]
fn google_introspection_answer_decode_is_pinned() {
    for (json, debug) in [
        (
            r#"{"email":"a@b.com","email_verified":"true","expires_in":"3599"}"#,
            r#"IntrospectedToken { email: Some("a@b.com"), email_verified: true, expires_in: 3599 }"#,
        ),
        (
            r#"{"email":"a@b.com","email_verified":true,"expires_in":42}"#,
            r#"IntrospectedToken { email: Some("a@b.com"), email_verified: true, expires_in: 42 }"#,
        ),
        (
            r#"{"email_verified":"false","expires_in":"soon"}"#,
            "IntrospectedToken { email: None, email_verified: false, expires_in: 0 }",
        ),
        (
            "{}",
            "IntrospectedToken { email: None, email_verified: false, expires_in: 0 }",
        ),
    ] {
        let token: IntrospectedToken = serde_json::from_str(json).unwrap();
        assert_eq!(format!("{token:?}"), debug, "{json}");
    }
}
