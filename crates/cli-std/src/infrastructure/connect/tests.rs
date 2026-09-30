use std::collections::HashMap;

use super::*;
use crate::connect::resolve_token;
use crate::domain::connect::{
    bearer_secrets, cr_tokens_secret, select_token, Role, TokenClaims, TOKEN_REGISTRY_SECRET_KEY,
};

#[test]
fn role_covers_hierarchy() {
    assert!(Role::Admin.covers(Role::Read));
    assert!(Role::Admin.covers(Role::Admin));
    assert!(!Role::Read.covers(Role::Admin));
    assert!(Role::Write.covers(Role::Read));
}

#[test]
fn select_token_picks_token_covering_role_for_collection_or_wildcard() {
    let mut registry = HashMap::new();
    registry.insert(
        "reader-token".to_string(),
        TokenClaims {
            subject: "reader".into(),
            roles: [("products".to_string(), Role::Read)].into_iter().collect(),
        },
    );
    registry.insert(
        "admin-token".to_string(),
        TokenClaims {
            subject: "admin".into(),
            roles: [("*".to_string(), Role::Admin)].into_iter().collect(),
        },
    );

    let picked = select_token(&registry, Role::Read, Some("products"));
    assert!(matches!(
        picked.as_deref(),
        Some("reader-token") | Some("admin-token")
    ));
    assert_eq!(
        select_token(&registry, Role::Admin, Some("products")).as_deref(),
        Some("admin-token"),
        "only the wildcard admin token covers admin on `products`"
    );

    let mut narrow = HashMap::new();
    narrow.insert(
        "scoped-token".to_string(),
        TokenClaims {
            subject: "scoped".into(),
            roles: [("orders".to_string(), Role::Write)].into_iter().collect(),
        },
    );
    assert!(select_token(&narrow, Role::Read, Some("products")).is_none());
}

/// #2678. The dev path — `port-forward` + read the Secret + pick a token —
/// has to keep working when a cluster's registry moves to the namespaced
/// shape. Both documents grant the same thing, so both must resolve the
/// same token; and `identities` must never be offered as a bearer secret,
/// since presenting an email is not proving an identity.
#[test]
fn both_registry_shapes_resolve_the_same_token() {
    let flat = br#"{"admin-token":{"subject":"ops","roles":{"*":"admin"}}}"#;
    let sectioned = br#"{"tokens":{"admin-token":{"subject":"ops","roles":{"*":"admin"}}},
                             "identities":{"dev@example.com":{"subject":"dev","roles":{"*":"admin"}}}}"#;

    for (label, bytes) in [("flat", &flat[..]), ("sectioned", &sectioned[..])] {
        let registry = bearer_secrets(bytes).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(
            select_token(&registry, Role::Admin, None).as_deref(),
            Some("admin-token"),
            "{label} registry resolves the admin token"
        );
        assert_eq!(
            registry.len(),
            1,
            "{label}: identities are not bearer secrets"
        );
    }

    // A flat registry whose only secret is spelled `tokens` stays flat —
    // the discriminator's second clause, shared with `service-auth`.
    let literal = br#"{"tokens":{"subject":"ops","roles":{"*":"admin"}}}"#;
    let registry = bearer_secrets(literal).unwrap();
    assert_eq!(
        select_token(&registry, Role::Admin, None).as_deref(),
        Some("tokens")
    );

    // An identities-only registry has no presentable credential at all.
    let identities_only =
        br#"{"identities":{"dev@example.com":{"subject":"dev","roles":{"*":"admin"}}}}"#;
    assert!(bearer_secrets(identities_only).unwrap().is_empty());
}

#[test]
fn cr_tokens_secret_reads_spec_field() {
    let cr = serde_json::json!({ "spec": { "tokensSecret": "svc-tokens" } });
    assert_eq!(cr_tokens_secret(&cr).as_deref(), Some("svc-tokens"));

    let cr_missing = serde_json::json!({ "spec": {} });
    assert_eq!(cr_tokens_secret(&cr_missing), None);
}

#[test]
fn decode_secret_data_decodes_base64_field() {
    use base64::Engine;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(b"{\"tok\":{\"subject\":\"s\"}}");
    let secret = serde_json::json!({ "data": { TOKEN_REGISTRY_SECRET_KEY: encoded } });
    let bytes = decode_secret_data(&secret, TOKEN_REGISTRY_SECRET_KEY).unwrap();
    assert_eq!(bytes, b"{\"tok\":{\"subject\":\"s\"}}");

    let missing = serde_json::json!({ "data": {} });
    assert!(decode_secret_data(&missing, TOKEN_REGISTRY_SECRET_KEY).is_err());
}

#[test]
fn wait_for_local_port_ready_succeeds_against_bound_listener() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    assert!(wait_for_local_port_ready(port, Duration::from_secs(2)).is_ok());
    drop(listener);
}

#[test]
fn wait_for_local_port_ready_times_out_against_closed_port() {
    let port = free_local_port().unwrap();
    assert!(wait_for_local_port_ready(port, Duration::from_millis(300)).is_err());
}

/// The process-management primitive, unit-tested with a real (but
/// harmless) child process instead of a live cluster's `kubectl
/// port-forward`.
#[test]
fn child_guard_kills_process_on_drop() {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "sleep 5"]);
    let guard = ChildGuard::spawn(&mut cmd).expect("spawn sleep");
    let pid = guard.child.id();
    drop(guard);
    std::thread::sleep(Duration::from_millis(200));
    let status = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("run kill -0");
    assert!(
        !status.success(),
        "process {pid} should be dead after ChildGuard drop"
    );
}

#[test]
fn child_guard_spawn_nonexistent_binary_errs() {
    let mut cmd = Command::new("cli-std-connect-test-nonexistent-binary-xyz-1376");
    assert!(ChildGuard::spawn(&mut cmd).is_err());
}

#[test]
fn resolve_token_prefers_explicit_token() {
    let resolved = resolve_token(Some("explicit"), None, None, None, Role::Read, None).unwrap();
    assert_eq!(resolved.as_deref(), Some("explicit"));
}

#[test]
fn resolve_token_returns_none_without_namespace_or_secret() {
    let resolved = resolve_token(None, None, None, None, Role::Read, None).unwrap();
    assert_eq!(resolved, None);
}
