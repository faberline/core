use axum::http::StatusCode;
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use service_auth::static_token::{AuthConfig, AuthEnv};
use service_auth::AuthError;

const ENV: AuthEnv = AuthEnv {
    mode: "TEST_AUTH",
    registry_file: "TEST_TOKEN_REGISTRY_FILE",
    legacy_tokens: "TEST_TOKENS",
};

const TOKENS: &str = r#"{"t1": {"subject": "alice", "roles": {"orders": "write", "*": "read"}}}"#;

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn old_resolve_empty_is_not_required_new_is_required() {
    let old = AuthConfig::resolve(ENV, "", None, None).unwrap();
    assert!(!old.required);

    let new = AuthConfig::resolve_required_by_default(ENV, "", None, Some(TOKENS)).unwrap();
    assert!(new.required);
    assert_eq!(new.tokens.len(), 1);
    assert_eq!(new.tokens["t1"].subject(), "alice");
}

#[test]
fn new_empty_mode_without_tokens_fails_fast_naming_the_env_vars() {
    let err = AuthConfig::resolve_required_by_default(ENV, "", None, None).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("TEST_TOKEN_REGISTRY_FILE"), "{message}");
    assert!(message.contains("TEST_TOKENS"), "{message}");
}

#[test]
fn open_off_and_disabled_are_not_required() {
    for mode in ["open", "off", "disabled", " OPEN "] {
        let config = AuthConfig::resolve_required_by_default(ENV, mode, None, None)
            .unwrap_or_else(|e| panic!("mode {mode:?} must resolve: {e}"));
        assert!(!config.required, "mode {mode:?} must not require auth");
        assert!(config.tokens.is_empty());
    }
}

#[test]
fn old_and_new_agree_on_required_and_on_invalid_modes() {
    for mode in ["required", "Required"] {
        let old = AuthConfig::resolve(ENV, mode, None, Some(TOKENS)).unwrap();
        let new = AuthConfig::resolve_required_by_default(ENV, mode, None, Some(TOKENS)).unwrap();
        assert!(old.required && new.required, "mode {mode:?}");
        let mut old_keys: Vec<&String> = old.tokens.keys().collect();
        let mut new_keys: Vec<&String> = new.tokens.keys().collect();
        old_keys.sort();
        new_keys.sort();
        assert_eq!(old_keys, new_keys, "mode {mode:?}");
    }

    for constructor in [
        AuthConfig::resolve(ENV, "maybe", None, None),
        AuthConfig::resolve_required_by_default(ENV, "maybe", None, None),
    ] {
        let message = format!("{:#}", constructor.unwrap_err());
        assert!(message.contains("TEST_AUTH"), "{message}");
        assert!(message.contains("`maybe`"), "{message}");
    }
}

#[test]
fn old_resolve_still_rejects_open() {
    let message = format!(
        "{:#}",
        AuthConfig::resolve(ENV, "open", None, None).unwrap_err()
    );
    assert!(message.contains("`open`"), "{message}");
}

#[tokio::test]
async fn unavailable_renders_auth_unavailable_by_default_and_unavailable_on_opt_in() {
    let default = AuthError::Unavailable("idp down".into()).into_response();
    assert_eq!(default.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body_json(default).await,
        serde_json::json!({"error": "auth_unavailable", "message": "idp down"})
    );

    let catalog = AuthError::Unavailable("idp down".into()).into_catalog_response();
    assert_eq!(catalog.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body_json(catalog).await,
        serde_json::json!({"error": "unavailable", "message": "idp down"})
    );
}

#[tokio::test]
async fn catalog_response_keeps_401_and_403_unchanged() {
    let unauthenticated = AuthError::Unauthenticated.into_catalog_response();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        body_json(unauthenticated).await,
        serde_json::json!({"error": "unauthenticated", "message": "valid bearer token required"})
    );

    let forbidden = AuthError::Forbidden("no".into()).into_catalog_response();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(forbidden).await,
        serde_json::json!({"error": "forbidden", "message": "no"})
    );
}
