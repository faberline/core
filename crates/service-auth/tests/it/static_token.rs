use service_auth::static_token::{authorize, AuthConfig, AuthEnv};
use service_auth::{AuthError, Role, RoleMapPrincipal};

const ENV: AuthEnv = AuthEnv {
    mode: "TEST_AUTH",
    registry_file: "TEST_TOKEN_REGISTRY_FILE",
    legacy_tokens: "TEST_TOKENS",
};

const TOKENS: &str = r#"{"t1": {"subject": "alice", "roles": {"orders": "write", "*": "read"}}}"#;

#[test]
fn open_config_is_not_required_and_has_no_tokens() {
    let config = AuthConfig::open();
    assert!(!config.required);
    assert!(config.tokens.is_empty());
}

#[test]
fn resolve_off_and_disabled_and_empty_do_not_require_auth() {
    for mode in ["off", "disabled", "", " OFF "] {
        let config = AuthConfig::resolve(ENV, mode, None, None)
            .unwrap_or_else(|e| panic!("mode {mode:?} must resolve: {e}"));
        assert!(!config.required, "mode {mode:?} must not require auth");
    }
}

#[test]
fn resolve_required_loads_the_legacy_inline_tokens() {
    let config = AuthConfig::resolve(ENV, "Required", None, Some(TOKENS)).unwrap();
    assert!(config.required);
    assert_eq!(config.tokens.len(), 1);
    assert_eq!(config.tokens["t1"].subject(), "alice");
}

#[test]
fn resolve_required_without_tokens_fails_fast_naming_the_env_vars() {
    // Required auth with no registry must fail at startup.
    let err = AuthConfig::resolve(ENV, "required", None, None).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("TEST_TOKEN_REGISTRY_FILE"), "{message}");
    assert!(message.contains("TEST_TOKENS"), "{message}");
}

#[test]
fn resolve_rejects_an_unknown_mode_naming_the_mode_env() {
    let err = AuthConfig::resolve(ENV, "maybe", None, None).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("TEST_AUTH"), "{message}");
    assert!(message.contains("`maybe`"), "{message}");
}

#[test]
fn resolve_reads_the_registry_file() {
    let path = std::env::temp_dir().join(format!(
        "static-token-resolve-reads-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, TOKENS).unwrap();
    let result = AuthConfig::resolve(ENV, "required", path.to_str(), None);
    let _ = std::fs::remove_file(&path);
    let config = result.unwrap();
    assert!(config.required);
    assert_eq!(config.tokens.len(), 1);
    assert_eq!(config.tokens["t1"].subject(), "alice");
}

#[test]
fn verifier_from_a_required_config_knows_the_tokens() {
    let verifier = AuthConfig::resolve(ENV, "required", None, Some(TOKENS))
        .unwrap()
        .verifier();
    assert_eq!(verifier.lookup_secret("t1").unwrap().subject(), "alice");
    assert!(verifier.lookup_secret("nope").is_none());
}

#[test]
fn authorize_allows_a_covering_role_and_denies_others_with_a_forbidden_message() {
    let verifier = AuthConfig::resolve(ENV, "required", None, Some(TOKENS))
        .unwrap()
        .verifier();
    let principal = verifier.audited(RoleMapPrincipal::Token(
        verifier.lookup_secret("t1").unwrap(),
    ));

    // Explicit grant: `orders: write`.
    assert!(authorize(&principal, "orders", Role::Write).is_ok());
    // Wildcard grant: `*: read`.
    assert!(authorize(&principal, "other", Role::Read).is_ok());
    // Denied: no `write` on `other`, only the wildcard `read`.
    match authorize(&principal, "other", Role::Write) {
        Err(AuthError::Forbidden(message)) => {
            assert_eq!(message, "subject `alice` lacks Write on `other`");
        }
        _ => panic!("expected AuthError::Forbidden for a missing write grant"),
    }
}

#[test]
fn authorize_in_open_mode_always_passes() {
    let verifier = AuthConfig::open().verifier();
    let principal = verifier.audited(RoleMapPrincipal::Open);
    assert!(authorize(&principal, "any", Role::Admin).is_ok());
}
