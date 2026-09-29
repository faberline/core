use super::*;

/// AC1's request half, without a cluster: the body that goes on the wire
/// carries exactly the audience and duration that were asked for, and the
/// path names exactly the namespace and ServiceAccount.
#[test]
fn the_request_names_the_audience_the_duration_the_namespace_and_the_account() {
    let target = target();
    assert_eq!(
        target.request_body(),
        json!({
            "apiVersion": "authentication.k8s.io/v1",
            "kind": "TokenRequest",
            "spec": {
                "audiences": ["callee.example.com"],
                "expirationSeconds": 600,
            }
        })
    );
    assert_eq!(
        target.subresource_path(),
        "/api/v1/namespaces/ops/serviceaccounts/app-client/token"
    );
}

#[test]
fn the_kubernetes_default_request_uses_empty_audiences_and_default_lifetime() {
    let target = TokenRequestTarget::kubernetes_default("ops", "app-client")
        .expect("valid Kubernetes target");
    assert_eq!(target.expiration_seconds(), DEFAULT_EXPIRATION_SECONDS);
    assert_eq!(
        target.subresource_path(),
        "/api/v1/namespaces/ops/serviceaccounts/app-client/token"
    );
    assert_eq!(
        target.request_body(),
        json!({
            "apiVersion": "authentication.k8s.io/v1",
            "kind": "TokenRequest",
            "spec": {
                "audiences": [],
                "expirationSeconds": 600,
            }
        })
    );
}

#[test]
fn the_kubernetes_default_constructor_rejects_invalid_names() {
    for (namespace, account) in [
        ("ops", "../../secrets/registry"),
        ("ops", "app-client/../lumen-operator"),
        ("ops/../kube-system", "app-client"),
        ("ops", ""),
        ("", "app-client"),
        ("ops", "App-Client"),
        ("ops", "-app-client"),
    ] {
        let err = TokenRequestTarget::kubernetes_default(namespace, account)
            .expect_err("invalid name must be rejected");
        assert!(
            matches!(err, TokenRequestError::InvalidTarget { .. }),
            "{namespace}/{account}: {err:?}"
        );
    }
}

#[test]
fn explicit_managed_and_custom_audiences_keep_their_wire_body() {
    for audience in ["managed.example.com", "custom.example.com"] {
        let target = TokenRequestTarget::new("ops", "app-client", audience)
            .expect("valid explicit audience");
        assert_eq!(
            target.request_body(),
            json!({
                "apiVersion": "authentication.k8s.io/v1",
                "kind": "TokenRequest",
                "spec": {
                    "audiences": [audience],
                    "expirationSeconds": 600,
                }
            })
        );
    }
}

/// The path string above is only worth asserting if it is the path `kube`
/// actually builds. This checks it against `kube`'s own derivation rather
/// than against a second copy of the same guess.
#[cfg(feature = "k8s")]
#[test]
fn the_documented_subresource_path_is_the_one_kube_derives() {
    use k8s_openapi::api::core::v1::ServiceAccount;
    use kube::Resource;

    let derived = format!(
        "{}/{}/token",
        ServiceAccount::url_path(&(), Some("ops")),
        "app-client"
    );
    assert_eq!(target().subresource_path(), derived);
}

#[test]
fn a_longer_lifetime_can_be_asked_for_and_a_shorter_one_cannot() {
    let longer = target()
        .with_expiration_seconds(3600)
        .expect("above the floor");
    assert_eq!(longer.request_body()["spec"]["expirationSeconds"], 3600);

    let err = target()
        .with_expiration_seconds(60)
        .expect_err("below the apiserver's floor");
    assert!(
        matches!(err, TokenRequestError::InvalidTarget { field, .. } if field == "expiration"),
        "{err:?}"
    );
    assert!(err.to_string().contains("600"), "{err}");
}

/// R3's other half: the target is never inferred, so every way of naming
/// it badly is refused rather than normalised into something that
/// addresses a different object.
#[test]
fn a_name_that_would_address_another_object_is_refused_before_anything_is_sent() {
    for (namespace, account) in [
        ("ops", "../../secrets/registry"),
        ("ops", "app-client/../lumen-operator"),
        ("ops/../kube-system", "app-client"),
        ("ops", ""),
        ("", "app-client"),
        ("ops", "App-Client"),
        ("ops", "-app-client"),
    ] {
        let err = TokenRequestTarget::new(namespace, account, AUDIENCE)
            .expect_err("a name that is not a Kubernetes name must not be accepted");
        assert!(
            matches!(err, TokenRequestError::InvalidTarget { .. }),
            "{namespace}/{account}: {err:?}"
        );
    }
}

#[test]
fn a_token_with_no_audience_is_refused() {
    let err =
        TokenRequestTarget::new("ops", "app-client", "  ").expect_err("an audience is required");
    assert!(
        matches!(err, TokenRequestError::InvalidTarget { field, .. } if field == "audience"),
        "{err:?}"
    );
}
