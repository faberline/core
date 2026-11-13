use super::*;
use std::path::Path;

fn managed() -> ManagedAuth {
    ManagedAuth::new("/etc/ca/ca.crt", [FileBearerScheme::Https]).unwrap()
}

fn file_bearer() -> FileBearerAuth {
    FileBearerAuth::new(
        "/private/tmp/service-token",
        ".example.internal",
        [FileBearerScheme::Https],
    )
    .unwrap()
}

/// The (path, contents) pairs of a generation result, in emitted order.
fn files(output: &GeneratedOutput) -> Vec<(String, String)> {
    output
        .files
        .iter()
        .map(|file| (file.rel_path.clone(), file.contents.clone()))
        .collect()
}

#[test]
fn managed_auth_accepts_https_only() {
    let auth = managed();
    assert_eq!(auth.ca_path(), Path::new("/etc/ca/ca.crt"));
    assert_eq!(
        auth.token_path(),
        "/var/run/secrets/kubernetes.io/serviceaccount/token"
    );
}

#[test]
fn managed_auth_refuses_empty_ca_path() {
    assert_eq!(
        ManagedAuth::new("", [FileBearerScheme::Https]),
        Err(ManagedAuthError::EmptyCaPath)
    );
}

#[test]
fn managed_auth_refuses_non_https_scheme() {
    assert_eq!(
        ManagedAuth::new("/etc/ca/ca.crt", [FileBearerScheme::Http]),
        Err(ManagedAuthError::NonHttpsScheme)
    );
    assert_eq!(
        ManagedAuth::new(
            "/etc/ca/ca.crt",
            [FileBearerScheme::Https, FileBearerScheme::Http]
        ),
        Err(ManagedAuthError::NonHttpsScheme)
    );
    assert_eq!(
        ManagedAuth::new("/etc/ca/ca.crt", Vec::<FileBearerScheme>::new()),
        Err(ManagedAuthError::NoSchemes)
    );
}

#[test]
fn generation_auth_refuses_both_profiles() {
    assert_eq!(
        GenerationAuth::new(Some(file_bearer()), Some(managed())),
        Err(GenerationAuthError::Conflict)
    );
    assert!(GenerationAuth::new(Some(file_bearer()), None).is_ok());
    assert!(GenerationAuth::new(None, Some(managed())).is_ok());
    assert!(GenerationAuth::new(None, None).is_ok());
}

#[test]
fn managed_rust_generation_changes_only_the_client() {
    // TypeScript (tests/it/managed_auth_ts.rs), Python (tests/it/managed_auth_py.rs)
    // and Rust (tests/it/managed_auth_rs.rs) all change with the Managed runtime.
    let auth = GenerationAuth::new(None, Some(managed())).unwrap();
    let opts = opts_for(Lang::Rust);
    let with_auth = files(&generate_with_auth(MINIMAL, &opts, &auth).unwrap());
    let legacy = files(&generate(MINIMAL, &opts).unwrap());
    let names = |files: &[(String, String)]| {
        files
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&with_auth), names(&legacy));
    for ((name, text), (_, legacy_text)) in with_auth.iter().zip(&legacy) {
        if name == "client.rs" {
            assert_ne!(text, legacy_text);
            assert!(text.contains("MANAGED_TOKEN_PATH"));
        } else {
            assert_eq!(text, legacy_text, "{name} must not change");
        }
    }
}

#[test]
fn file_bearer_only_generation_matches_existing_entry() {
    let token = file_bearer();
    let auth = GenerationAuth::new(Some(token.clone()), None).unwrap();
    let opts = opts_for(Lang::Ts);
    let with_auth = generate_with_auth(MINIMAL, &opts, &auth).unwrap();
    let direct = generate_with_file_bearer_auth(MINIMAL, &opts, &token).unwrap();
    assert_eq!(files(&with_auth), files(&direct));
}
