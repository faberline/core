use super::*;

// -- #2764: two sources, two confidentiality classes -------------------

/// A scratch directory per test. Sharing one would let a parallel test's
/// cleanup delete a file this one is mid-read on.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("service-auth-2764-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &std::path::Path, file: &str, body: &str) -> String {
    let path = dir.join(file);
    std::fs::write(&path, body).unwrap();
    path.to_str().unwrap().to_owned()
}

/// #2764's structural claim. The identity map is a ConfigMap and the
/// bearer secrets are a Secret; two Kubernetes objects cannot share one
/// mount path, so the loader must union two files. If it could not, the
/// only way to serve both classes would be to put the plaintext identity
/// map back inside the Secret — exactly the coupling this work removes.
#[test]
fn two_files_union_into_one_registry_each_keeping_its_own_namespace() {
    let dir = scratch("union");
    let identities = write(
        &dir,
        "identities.json",
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"products":"read"}}}}"#,
    );
    let tokens = write(
        &dir,
        "token-registry.json",
        r#"{"tokens":{"s3cret":{"subject":"svc","roles":{"products":"write"}}}}"#,
    );

    let registry = load_registry_files(
        true,
        &[
            RegistrySource {
                env: "IDENTITY_FILE",
                path: Some(&identities),
            },
            RegistrySource {
                env: "TOKEN_FILE",
                path: Some(&tokens),
            },
        ],
    )
    .expect("two sources union");

    assert_eq!(registry.identities["a@b.com"].subject, "dev");
    assert_eq!(registry.tokens["s3cret"].subject, "svc");
    // Still disjoint after the merge — the union is per namespace.
    assert!(!registry.identities.contains_key("s3cret"));
    assert!(!registry.tokens.contains_key("a@b.com"));
}

/// Last-writer-wins would reproduce the failure #2764 quotes as the reason
/// to delete the mutual-exclusion CEL rule: no way to tell which registry
/// is actually being served. Two sources disagreeing about one principal's
/// grants is a deployment mistake, and it fails loudly.
#[test]
fn a_key_claimed_by_two_sources_is_an_error_not_a_silent_overwrite() {
    let dir = scratch("collision");
    let first = write(
        &dir,
        "a.json",
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"products":"read"}}}}"#,
    );
    let second = write(
        &dir,
        "b.json",
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"products":"admin"}}}}"#,
    );

    let err = load_registry_files(
        true,
        &[
            RegistrySource {
                env: "FIRST",
                path: Some(&first),
            },
            RegistrySource {
                env: "SECOND",
                path: Some(&second),
            },
        ],
    )
    .unwrap_err();

    let message = format!("{err:#}");
    assert!(message.contains("a@b.com"), "{message}");
    assert!(message.contains("SECOND"), "{message}");
}

/// The collision message is read by whoever is debugging the deployment,
/// which usually means it lands in a log aggregator. A `tokens` key is the
/// bearer secret itself, so it is named by the subject it grants.
#[test]
fn a_token_collision_names_the_subject_and_never_the_secret() {
    let mut registry =
        Registry::parse(r#"{"tokens":{"s3cret":{"subject":"svc","roles":{}}}}"#).unwrap();
    let other = Registry::parse(r#"{"tokens":{"s3cret":{"subject":"svc","roles":{}}}}"#).unwrap();

    let message = registry.try_merge(other).unwrap_err().to_string();
    assert!(message.contains("svc"), "{message}");
    assert!(
        !message.contains("s3cret"),
        "the collision message leaked a bearer secret: {message}"
    );
}

/// The same key in *different* namespaces is not a collision: one is a
/// secret and one is an email, and they are resolved by different lookups.
#[test]
fn the_same_key_in_two_namespaces_is_not_a_collision() {
    let mut registry =
        Registry::parse(r#"{"tokens":{"shared":{"subject":"svc","roles":{}}}}"#).unwrap();
    let other =
        Registry::parse(r#"{"identities":{"shared":{"subject":"dev","roles":{}}}}"#).unwrap();
    registry.try_merge(other).expect("different namespaces");
    assert_eq!(registry.tokens["shared"].subject, "svc");
    assert_eq!(registry.identities["shared"].subject, "dev");
}

/// An absent source is absent, not empty. A service configured with only
/// an identity map must not be told its registry file failed to read.
#[test]
fn unset_and_blank_sources_are_skipped() {
    let dir = scratch("skip");
    let identities = write(
        &dir,
        "identities.json",
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{}}}}"#,
    );

    let registry = load_registry_files(
        true,
        &[
            RegistrySource {
                env: "IDENTITY_FILE",
                path: Some(&identities),
            },
            RegistrySource {
                env: "TOKEN_FILE",
                path: None,
            },
            RegistrySource {
                env: "LEGACY_FILE",
                path: Some("   "),
            },
        ],
    )
    .expect("one configured source is enough");
    assert_eq!(registry.len(), 1);
}

/// Fail-fast names every source the operator could have set, because the
/// mistake is usually "I set the other one".
#[test]
fn required_with_no_source_at_all_names_every_env_var() {
    let err = load_registry_files(
        true,
        &[
            RegistrySource {
                env: "IDENTITY_FILE",
                path: None,
            },
            RegistrySource {
                env: "TOKEN_FILE",
                path: None,
            },
        ],
    )
    .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("IDENTITY_FILE"), "{message}");
    assert!(message.contains("TOKEN_FILE"), "{message}");
}

/// #2679's half of the invariant. The control plane presents an identity
/// of its own; a tenant who could grant that same subject to a credential
/// they hold would be impersonating the operator, and every audit line
/// would name the operator rather than them.
#[test]
fn a_registry_claiming_a_reserved_subject_is_reported_with_its_section_and_key() {
    let registry = Registry::parse(
        r#"{"identities":{"tenant@b.com":{"subject":"lumen-control-plane","roles":{}}}}"#,
    )
    .unwrap();

    let (section, key, subject) = registry
        .reserved_subject_violation(&["lumen-control-plane".to_owned()])
        .expect("the reserved subject is claimed");
    assert_eq!(section, "identities");
    assert_eq!(key, "tenant@b.com");
    assert_eq!(subject, "lumen-control-plane");

    assert!(registry
        .reserved_subject_violation(&["someone-else".to_owned()])
        .is_none());
    assert!(registry.reserved_subject_violation(&[]).is_none());
}
