use super::*;

// -- #2678: the two namespaces ----------------------------------------

const NAMESPACED: &str = r#"{
        "tokens":     { "s3cret":  { "subject": "svc", "roles": { "products": "write" } } },
        "identities": { "a@b.com": { "subject": "dev", "roles": { "products": "read"  } } }
    }"#;

#[test]
fn namespaced_document_lands_each_entry_in_its_own_namespace() {
    let registry = Registry::parse(NAMESPACED).unwrap();
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.tokens["s3cret"].subject(), "svc");
    assert_eq!(registry.identities["a@b.com"].subject(), "dev");
    // The whole point: neither key resolves from the other's map.
    assert!(!registry.tokens.contains_key("a@b.com"));
    assert!(!registry.identities.contains_key("s3cret"));
}

#[test]
fn a_flat_document_is_read_as_bearer_secrets() {
    let registry = Registry::parse(r#"{"s3cret":{"subject":"svc","roles":{"*":"read"}}}"#).unwrap();
    assert_eq!(registry.tokens.len(), 1);
    assert!(registry.identities.is_empty());
}

/// The discriminator is "all keys are section names **and** no top-level
/// value is a claims object", not just the first clause — otherwise a flat
/// registry whose bearer secret is literally the word `tokens` would be
/// misread as an empty section and silently authenticate nobody.
#[test]
fn a_flat_document_whose_secret_is_spelled_tokens_is_still_flat() {
    let registry = Registry::parse(r#"{"tokens":{"subject":"svc","roles":{"*":"read"}}}"#).unwrap();
    assert_eq!(registry.tokens["tokens"].subject(), "svc");
    assert!(registry.identities.is_empty());
}

#[test]
fn a_partial_namespaced_document_leaves_the_absent_section_empty() {
    let registry =
        Registry::parse(r#"{"identities":{"a@b.com":{"subject":"dev","roles":{}}}}"#).unwrap();
    assert!(registry.tokens.is_empty());
    assert_eq!(registry.identities.len(), 1);
}

/// A bearer-only service that was handed identities cannot resolve them.
/// Dropping them silently would present to the operator as an unexplained
/// 401 on a credential the registry appears to grant.
#[test]
fn bearer_only_loader_rejects_a_document_carrying_identities() {
    let dir = std::env::temp_dir().join("service-auth-2678-identities");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("registry.json");
    std::fs::write(&path, NAMESPACED).unwrap();

    let err = load_registry(
        true,
        "REGISTRY_FILE",
        Some(path.to_str().unwrap()),
        "LEGACY_TOKENS",
        None,
    )
    .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("REGISTRY_FILE"), "{message}");
    assert!(message.contains("identities"), "{message}");
    std::fs::remove_file(&path).ok();
}

#[test]
fn identity_only_registry_satisfies_required_for_an_identity_aware_service() {
    let dir = std::env::temp_dir().join("service-auth-2678-identity-only");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("registry.json");
    std::fs::write(
        &path,
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"*":"read"}}}}"#,
    )
    .unwrap();

    let registry = load_registry_file(true, "REGISTRY_FILE", Some(path.to_str().unwrap()))
        .expect("an identity-only registry can authenticate someone");
    assert_eq!(registry.identities.len(), 1);
    assert!(registry.tokens.is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn identity_aware_loader_fails_fast_when_required_and_both_namespaces_are_empty() {
    let err = load_registry_file(true, "REGISTRY_FILE", None).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("REGISTRY_FILE"), "{message}");
    assert!(message.contains("identities"), "{message}");
}
