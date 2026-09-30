use std::sync::Mutex;
use std::time::Duration;

use axum::http::header;

use super::*;
use crate::reload::*;
use crate::Role;

#[derive(Default)]
struct RecordingSink(Mutex<Vec<AuthEvent>>);

impl AuthEventSink for RecordingSink {
    fn record(&self, event: &AuthEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

fn claims(subject: &str, role: Role) -> TokenClaims {
    TokenClaims::new(
        subject.to_owned(),
        HashMap::from([("resource".to_owned(), role)]),
    )
}

fn headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    headers
}

#[test]
fn valid_rotation_is_immediately_visible_and_advances_revision() {
    let verifier = ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Read))]),
    );
    assert_eq!(
        verifier.authenticate(&headers("old")).unwrap().subject(),
        Some("alice")
    );
    let revision = verifier
        .reload_json(r#"{"new":{"subject":"bob","roles":{"resource":"write"}}}"#)
        .unwrap();
    assert_eq!(revision, 1);
    assert!(verifier.authenticate(&headers("old")).is_err());
    let principal = verifier.authenticate(&headers("new")).unwrap();
    assert_eq!(principal.subject(), Some("bob"));
    assert!(principal.ensure("resource", Role::Write).is_ok());
}

#[test]
fn invalid_replacements_preserve_last_known_good_snapshot() {
    let verifier = ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Admin))]),
    );
    assert!(verifier.reload_json("not-json").is_err());
    assert!(verifier.reload_json("{}").is_err());
    assert!(verifier
        .reload_json(r#"{"new":{"subject":"","roles":{}}}"#)
        .is_err());
    assert_eq!(verifier.revision(), 0);
    assert_eq!(verifier.entry_count(), 1);
    assert!(verifier.authenticate(&headers("old")).is_ok());
}

// -- #2678 AC5: rotation covers both namespaces ------------------------

/// A rotation that adds an identity is visible to `lookup_identity` and
/// stays invisible to the bearer path — the reload-time half of the same
/// disjointness the verifier enforces per request.
#[test]
fn rotation_can_add_identities_without_widening_the_bearer_namespace() {
    let verifier = ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("s3cret".to_owned(), claims("svc", Role::Admin))]),
    );

    verifier
        .reload_json(
            r#"{"tokens":{"s3cret":{"subject":"svc","roles":{"*":"admin"}}},
                    "identities":{"a@b.com":{"subject":"dev","roles":{"*":"read"}}}}"#,
        )
        .unwrap();

    assert_eq!(verifier.entry_count(), 2);
    assert_eq!(
        verifier.lookup_identity("a@b.com").unwrap().subject(),
        "dev"
    );
    assert!(verifier.lookup_secret("a@b.com").is_none());
    assert!(
        verifier.authenticate(&headers("a@b.com")).is_err(),
        "presenting the email as a bearer secret must not authenticate"
    );
}

/// AC5. A malformed identity-keyed rotation is rejected whole — the
/// previous registry keeps serving rather than half-applying. An identity
/// key that is not an email address is the specific malformation an
/// operator hits by pasting a bearer secret into the wrong section.
#[test]
fn a_malformed_identity_rotation_leaves_the_previous_registry_serving() {
    let verifier = ReloadableRoleMapVerifier::with_registry(
        true,
        Registry {
            tokens: HashMap::from([("s3cret".to_owned(), claims("svc", Role::Admin))]),
            identities: HashMap::from([("a@b.com".to_owned(), claims("dev", Role::Read))]),
        },
    );

    for bad in [
        // an identity key that is not an email — a pasted bearer secret
        r#"{"identities":{"not-an-email":{"subject":"dev","roles":{"*":"read"}}}}"#,
        // an identity entry with no subject to audit
        r#"{"identities":{"a@b.com":{"subject":"","roles":{"*":"read"}}}}"#,
        // the section is present but is not a map of claims
        r#"{"identities":["a@b.com"]}"#,
    ] {
        assert!(verifier.reload_json(bad).is_err(), "accepted `{bad}`");
    }

    assert_eq!(verifier.revision(), 0);
    assert_eq!(verifier.entry_count(), 2);
    assert_eq!(
        verifier.lookup_identity("a@b.com").unwrap().subject(),
        "dev"
    );
    assert!(verifier.authenticate(&headers("s3cret")).is_ok());
}

#[test]
fn authorization_events_are_typed_and_credential_free() {
    let sink = Arc::new(RecordingSink::default());
    let verifier = ReloadableRoleMapVerifier::with_sink(
        true,
        HashMap::from([("supersecret".to_owned(), claims("alice", Role::Read))]),
        sink.clone(),
    );
    let principal = verifier.authenticate(&headers("supersecret")).unwrap();
    assert!(principal.ensure("resource", Role::Write).is_err());
    assert!(verifier.authenticate(&headers("another-secret")).is_err());

    let events = sink.0.lock().unwrap();
    let json = serde_json::to_string(&*events).unwrap();
    assert!(!json.contains("supersecret"));
    assert!(!json.contains("another-secret"));
    assert!(!json.contains("\"token\""));
    assert!(!json.contains("\"credential\""));
    assert!(events.iter().any(|event| matches!(
        event,
        AuthEvent::AuthorizationDecision {
            decision: AuthorizationDecision::Deny,
            reason: AuthorizationReason::InsufficientRole,
            subject: Some(subject),
            ..
        } if subject == "alice"
    )));
}

#[test]
fn failed_file_reload_emits_read_failure_without_losing_registry() {
    let sink = Arc::new(RecordingSink::default());
    let verifier = ReloadableRoleMapVerifier::with_sink(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Read))]),
        sink.clone(),
    );
    assert!(verifier
        .reload_file("/definitely/missing/registry.json")
        .is_err());
    assert!(verifier.authenticate(&headers("old")).is_ok());
    assert!(sink.0.lock().unwrap().iter().any(|event| matches!(
        event,
        AuthEvent::RegistryReload {
            applied: false,
            failure: Some(ReloadFailure::Read),
            ..
        }
    )));
}

#[tokio::test]
async fn file_watcher_adopts_a_valid_replacement_without_restart() {
    let path = std::env::temp_dir().join(format!(
        "service-auth-registry-watch-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"{"old":{"subject":"alice","roles":{"resource":"read"}}}"#,
    )
    .unwrap();
    let verifier = Arc::new(ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Read))]),
    ));
    let task = spawn_registry_file_watcher_with_interval(
        Arc::clone(&verifier),
        &path,
        Duration::from_millis(5),
    );

    // Let the task capture the initial file before replacing the mounted
    // content, then wait until the shared verifier publishes the change.
    tokio::task::yield_now().await;
    std::fs::write(
        &path,
        r#"{"new":{"subject":"bob","roles":{"resource":"admin"}}}"#,
    )
    .unwrap();
    for _ in 0..20 {
        if verifier.authenticate(&headers("new")).is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert!(verifier.authenticate(&headers("old")).is_err());
    assert_eq!(
        verifier.authenticate(&headers("new")).unwrap().subject(),
        Some("bob")
    );
    task.abort();
    std::fs::remove_file(path).ok();
}

// -- #2764: reloading a registry that arrives as two files -------------

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("service-auth-reload-2764-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The trap this test exists for: reloading only the file that changed
/// would publish a snapshot containing that file alone, silently dropping
/// the other namespace. A ConfigMap edit would revoke every bearer secret.
#[test]
fn reloading_one_of_two_files_keeps_the_other_file_serving() {
    let dir = scratch("both");
    let identities = dir.join("identities.json");
    let tokens = dir.join("token-registry.json");
    std::fs::write(
        &identities,
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"resource":"read"}}}}"#,
    )
    .unwrap();
    std::fs::write(
        &tokens,
        r#"{"tokens":{"s3cret":{"subject":"svc","roles":{"resource":"write"}}}}"#,
    )
    .unwrap();

    let verifier = ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("s3cret".to_owned(), claims("svc", Role::Write))]),
    );
    let paths = vec![identities.clone(), tokens.clone()];
    verifier.reload_files(&paths).unwrap();
    assert!(verifier.authenticate(&headers("s3cret")).is_ok());
    assert!(verifier.lookup_identity("a@b.com").is_some());

    // Edit only the ConfigMap half.
    std::fs::write(
        &identities,
        r#"{"identities":{"c@d.com":{"subject":"ops","roles":{"resource":"admin"}}}}"#,
    )
    .unwrap();
    verifier.reload_files(&paths).unwrap();

    assert!(verifier.lookup_identity("a@b.com").is_none());
    assert!(verifier.lookup_identity("c@d.com").is_some());
    assert!(
        verifier.authenticate(&headers("s3cret")).is_ok(),
        "editing the identity map must not revoke the bearer secrets"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// All-or-nothing across sources. A half-written ConfigMap must not take
/// the Secret's entries down with it.
#[test]
fn one_unreadable_source_leaves_the_previous_snapshot_serving() {
    let dir = scratch("partial");
    let good = dir.join("identities.json");
    std::fs::write(
        &good,
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"resource":"read"}}}}"#,
    )
    .unwrap();

    let sink = Arc::new(RecordingSink::default());
    let verifier = ReloadableRoleMapVerifier::with_sink(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Read))]),
        sink.clone(),
    );

    let err = verifier
        .reload_files(&[good.clone(), dir.join("does-not-exist.json")])
        .unwrap_err();
    assert!(format!("{err:#}").contains("does-not-exist"), "{err:#}");

    assert!(verifier.authenticate(&headers("old")).is_ok());
    assert!(
        verifier.lookup_identity("a@b.com").is_none(),
        "the readable half must not be published on its own"
    );
    assert!(sink.0.lock().unwrap().iter().any(|event| matches!(
        event,
        AuthEvent::RegistryReload {
            applied: false,
            failure: Some(ReloadFailure::Read),
            ..
        }
    )));

    std::fs::remove_dir_all(&dir).ok();
}

/// #2679. A tenant-editable ConfigMap that grants the control plane's own
/// subject is rejected at reload, not at request time: accepting it would
/// let a tenant credential act as the operator and sign every audit line
/// with the operator's name.
#[test]
fn a_replacement_claiming_the_reserved_subject_is_refused() {
    let verifier = ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("old".to_owned(), claims("alice", Role::Read))]),
    )
    .reserving_subjects(["lumen-control-plane".to_owned()]);
    assert_eq!(verifier.reserved_subjects(), ["lumen-control-plane"]);

    let err = verifier
        .reload_json(
            r#"{"identities":{"tenant@b.com":{"subject":"lumen-control-plane","roles":{"resource":"admin"}}}}"#,
        )
        .unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("lumen-control-plane"), "{message}");
    assert!(message.contains("tenant@b.com"), "{message}");

    assert_eq!(verifier.revision(), 0);
    assert!(verifier.authenticate(&headers("old")).is_ok());
}

/// The watcher's contract when the registry spans two files: a change to
/// either one republishes the union.
#[tokio::test]
async fn the_multi_file_watcher_republishes_the_union_on_any_change() {
    let dir = scratch(&format!("watch-{}", std::process::id()));
    let identities = dir.join("identities.json");
    let tokens = dir.join("token-registry.json");
    std::fs::write(&identities, r#"{"identities":{}}"#).unwrap();
    std::fs::write(
        &tokens,
        r#"{"tokens":{"s3cret":{"subject":"svc","roles":{"resource":"write"}}}}"#,
    )
    .unwrap();

    let verifier = Arc::new(ReloadableRoleMapVerifier::new(
        true,
        HashMap::from([("s3cret".to_owned(), claims("svc", Role::Write))]),
    ));
    let task = spawn_registry_files_watcher_with_interval(
        Arc::clone(&verifier),
        &[identities.clone(), tokens.clone()],
        Duration::from_millis(5),
    );

    tokio::task::yield_now().await;
    std::fs::write(
        &identities,
        r#"{"identities":{"a@b.com":{"subject":"dev","roles":{"resource":"read"}}}}"#,
    )
    .unwrap();
    for _ in 0..40 {
        if verifier.lookup_identity("a@b.com").is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert!(verifier.lookup_identity("a@b.com").is_some());
    assert!(
        verifier.authenticate(&headers("s3cret")).is_ok(),
        "the untouched file stays in the published snapshot"
    );

    task.abort();
    std::fs::remove_dir_all(&dir).ok();
}
