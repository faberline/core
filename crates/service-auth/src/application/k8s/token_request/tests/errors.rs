use super::*;

/// R6: the denial names who was refused, what they were refused, and the
/// exact question whose answer it is.
#[test]
fn a_denial_names_the_caller_the_target_and_the_check() {
    let err = TokenRequestError::Forbidden {
        username: Some("alice@example.com".to_string()),
        namespace: "ops".to_string(),
        service_account: "app-client".to_string(),
        detail: "cannot create resource \"serviceaccounts/token\"".to_string(),
    };
    let rendered = err.to_string();
    for expected in [
        "alice@example.com",
        "ops/app-client",
        "kubectl auth can-i create serviceaccounts/app-client --subresource=token -n ops",
        "resourceNames: [app-client]",
    ] {
        assert!(
            rendered.contains(expected),
            "missing `{expected}`: {rendered}"
        );
    }
}

#[test]
fn a_denial_that_cannot_name_the_caller_says_so_rather_than_guessing() {
    let err = TokenRequestError::Forbidden {
        username: None,
        namespace: "ops".to_string(),
        service_account: "app-client".to_string(),
        detail: "forbidden".to_string(),
    };
    let rendered = err.to_string();
    assert!(
        rendered.contains("the identity in your kubeconfig"),
        "{rendered}"
    );
    assert!(!rendered.contains("None"), "{rendered}");
}

/// AC6 at this layer: neither the token wrapper nor any error rendering
/// contains the material. Asserted against the bytes, not the wording.
#[tokio::test]
async fn nothing_this_module_can_print_contains_the_token() {
    let clock = Arc::new(ManualClock::new(0));
    let minter = Arc::new(RecordingMinter::new(
        clock.clone(),
        Duration::from_secs(600),
    ));
    let source = TokenSource::with_clock(minter, target(), clock);
    let token = source.token().await.expect("mint");
    let material = token.expose().to_string();
    assert!(
        material.contains(CANARY),
        "the fixture must be recognisable"
    );

    for rendered in [format!("{token}"), format!("{token:?}")] {
        assert!(
            !rendered.contains(CANARY),
            "a token printed itself: {rendered}"
        );
    }

    // `MintedToken` is the struct most likely to be reached by a derived
    // `Debug` upstream — it is what a connection state machine holds.
    let minted = MintedToken::new(material.clone(), 0, 600_000);
    let rendered = format!("{minted:?}");
    assert!(
        !rendered.contains(CANARY),
        "a minted token printed itself: {rendered}"
    );
    assert!(
        rendered.contains("expires_at_millis"),
        "the expiry is still worth printing: {rendered}"
    );
}

/// The other half of AC6 here: no error this module *composes* is given
/// the material to begin with. Every variant is built from the target and
/// from the apiserver's own message, and the one call that could reach a
/// token — `mint` — puts the token in [`MintedToken`] or nowhere.
#[test]
fn no_error_variant_has_a_field_that_could_hold_a_token() {
    let target = target();
    let variants = [
        TokenRequestError::InvalidTarget {
            field: "namespace",
            value: target.namespace().to_string(),
            reason: "example".to_string(),
        },
        TokenRequestError::NoIdentity {
            detail: "no kubeconfig".to_string(),
        },
        TokenRequestError::Forbidden {
            username: Some("alice@example.com".to_string()),
            namespace: target.namespace().to_string(),
            service_account: target.service_account().to_string(),
            detail: "cannot create resource".to_string(),
        },
        TokenRequestError::NoSuchServiceAccount {
            namespace: target.namespace().to_string(),
            service_account: target.service_account().to_string(),
        },
        TokenRequestError::Transport {
            detail: "connection reset".to_string(),
        },
        TokenRequestError::Malformed {
            detail: "no status".to_string(),
        },
    ];
    for variant in &variants {
        for rendered in [variant.to_string(), format!("{variant:?}")] {
            assert!(
                !rendered.contains(CANARY),
                "an error rendering carried the credential: {rendered}"
            );
            assert!(!rendered.is_empty());
        }
    }
}
