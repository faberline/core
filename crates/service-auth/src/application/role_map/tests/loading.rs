use super::*;

#[test]
fn load_registry_empty_when_neither_source_set() {
    let tokens = load_registry(false, "REGISTRY_FILE", None, "LEGACY_TOKENS", None).unwrap();
    assert!(tokens.is_empty());
}

#[test]
fn load_registry_prefers_file_over_legacy_json() {
    let path = std::env::temp_dir().join(format!(
        "service-auth-role-map-test-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"{"file-token": {"subject": "alice", "roles": {"u": "write"}}}"#,
    )
    .unwrap();
    let tokens = load_registry(
        true,
        "REGISTRY_FILE",
        Some(path.to_str().unwrap()),
        "LEGACY_TOKENS",
        Some(r#"{"env-token": {"subject": "env", "roles": {"*": "admin"}}}"#),
    )
    .unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get("file-token").unwrap().subject(), "alice");
    assert!(tokens.get("env-token").is_none());
}

#[test]
fn load_registry_falls_back_to_legacy_json() {
    let tokens = load_registry(
        true,
        "REGISTRY_FILE",
        None,
        "LEGACY_TOKENS",
        Some(r#"{"t1": {"subject": "alice", "roles": {"u": "write"}}}"#),
    )
    .unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get("t1").unwrap().subject(), "alice");
}

#[test]
fn load_registry_required_without_tokens_fails_fast() {
    let err = load_registry(true, "REGISTRY_FILE", None, "LEGACY_TOKENS", None).unwrap_err();
    assert!(err.to_string().contains("REGISTRY_FILE"));
    assert!(err.to_string().contains("LEGACY_TOKENS"));
}

#[test]
fn load_registry_rejects_bad_json() {
    let err = load_registry(
        false,
        "REGISTRY_FILE",
        None,
        "LEGACY_TOKENS",
        Some("not-json"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("LEGACY_TOKENS"));
}
