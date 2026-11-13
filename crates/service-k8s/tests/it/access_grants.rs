#![cfg(feature = "controller")]

use serde_json::json;
use service_k8s::service::ConditionStatus;
use service_k8s::{
    access_policy_condition, render_access_grants, validate_policy, AccessGrant, AccessGrantError,
    AccessRule, AccessSubject, GrantOutcome,
};

fn sample_rule() -> AccessRule {
    AccessRule::new(
        vec!["apps".into()],
        vec!["deployments".into()],
        vec!["my-app".into()],
        vec!["get".into(), "update".into()],
    )
}

fn sample_subject() -> AccessSubject {
    AccessSubject::new("app-ns", "app-sa")
}

fn valid_grant(name: &str) -> AccessGrant {
    AccessGrant::new("app-ns", name, vec![sample_rule()], vec![sample_subject()])
}

#[test]
fn wildcard_in_verbs_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].verbs = vec!["*".into()];
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::Wildcard {
            grant: "app-access".into(),
            path: "rules/0/verbs/0".into(),
        })
    );
}

#[test]
fn wildcard_in_resources_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resources = vec!["*".into()];
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::Wildcard {
            grant: "app-access".into(),
            path: "rules/0/resources/0".into(),
        })
    );
}

#[test]
fn wildcard_in_api_groups_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].api_groups = vec!["*".into()];
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::Wildcard {
            grant: "app-access".into(),
            path: "rules/0/apiGroups/0".into(),
        })
    );
}

#[test]
fn wildcard_in_resource_names_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resource_names = vec!["*".into()];
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::Wildcard {
            grant: "app-access".into(),
            path: "rules/0/resourceNames/0".into(),
        })
    );
}

#[test]
fn wildcard_in_subject_name_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.subjects[0].name = "*".into();
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::Wildcard {
            grant: "app-access".into(),
            path: "subjects/0/name".into(),
        })
    );
}

#[test]
fn empty_resource_names_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resource_names = vec![];
    assert_eq!(
        validate_policy(&[grant]),
        Err(AccessGrantError::EmptyResourceNames {
            grant: "app-access".into(),
        })
    );
}

#[test]
fn exact_subjects_and_rules_render_byte_for_byte() {
    let grant = AccessGrant::new(
        "app-ns",
        "app-access",
        vec![sample_rule()],
        vec![
            AccessSubject::new("app-ns", "app-sa"),
            AccessSubject::new("other-ns", "other-sa"),
        ],
    );
    let rendered = serde_json::Value::Array(render_access_grants(&[grant]));
    assert_eq!(
        rendered,
        json!([
            {
                "apiVersion": "rbac.authorization.k8s.io/v1",
                "kind": "Role",
                "metadata": {"name": "app-access", "namespace": "app-ns", "labels": {}},
                "rules": [{
                    "apiGroups": ["apps"],
                    "resources": ["deployments"],
                    "verbs": ["get", "update"],
                    "resourceNames": ["my-app"]
                }]
            },
            {
                "apiVersion": "rbac.authorization.k8s.io/v1",
                "kind": "RoleBinding",
                "metadata": {"name": "app-access", "namespace": "app-ns", "labels": {}},
                "roleRef": {
                    "apiGroup": "rbac.authorization.k8s.io",
                    "kind": "Role",
                    "name": "app-access"
                },
                "subjects": [
                    {"kind": "ServiceAccount", "name": "app-sa", "namespace": "app-ns"},
                    {"kind": "ServiceAccount", "name": "other-sa", "namespace": "other-ns"}
                ]
            }
        ])
    );
    let binding = &rendered[1];
    assert!(binding["metadata"].get("ownerReferences").is_none());
    assert_eq!(binding["roleRef"]["kind"], "Role");
}

#[test]
fn render_order_is_stable_under_shuffled_input() {
    let a = AccessGrant::new("ns-b", "g2", vec![sample_rule()], vec![sample_subject()]);
    let b = AccessGrant::new("ns-a", "g9", vec![sample_rule()], vec![sample_subject()]);
    let c = AccessGrant::new("ns-a", "g1", vec![sample_rule()], vec![sample_subject()]);

    let forward = render_access_grants(&[a.clone(), b.clone(), c.clone()]);
    let shuffled = render_access_grants(&[c.clone(), a.clone(), b.clone()]);
    assert_eq!(forward, shuffled);

    let roles: Vec<(String, String)> = forward
        .iter()
        .filter(|item| item["kind"] == "Role")
        .map(|item| {
            (
                item["metadata"]["namespace"].as_str().unwrap().to_string(),
                item["metadata"]["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        roles,
        vec![
            ("ns-a".to_string(), "g1".to_string()),
            ("ns-a".to_string(), "g9".to_string()),
            ("ns-b".to_string(), "g2".to_string()),
        ]
    );
}

#[test]
fn empty_set_is_deny_all_and_passes() {
    assert_eq!(validate_policy(&[]), Ok(()));
    assert!(render_access_grants(&[]).is_empty());
    let fact = access_policy_condition(&[], &[]);
    assert_eq!(fact.status, ConditionStatus::True);
    assert_eq!(fact.reason, "DenyAll");
}

#[test]
fn duplicate_grant_refused() {
    assert_eq!(
        validate_policy(&[valid_grant("dup"), valid_grant("dup")]),
        Err(AccessGrantError::DuplicateGrant {
            grant: "dup".into(),
            namespace: "app-ns".into(),
        })
    );
}

#[test]
fn access_policy_ready_condition_lifecycle_and_partial_failure() {
    let grants = [valid_grant("grant-one"), valid_grant("grant-two")];

    let partial = access_policy_condition(
        &grants,
        &[
            GrantOutcome::applied("app-ns", "grant-one"),
            GrantOutcome::failed("app-ns", "grant-two"),
        ],
    );
    assert_eq!(partial.type_, "AccessPolicyReady");
    assert_eq!(partial.status, ConditionStatus::False);
    assert_eq!(partial.reason, "ApplyFailed");
    assert!(partial.message.contains("grant-two"));
    assert!(!partial.message.contains("grant-one"));

    let recovered = access_policy_condition(
        &grants,
        &[
            GrantOutcome::applied("app-ns", "grant-one"),
            GrantOutcome::applied("app-ns", "grant-two"),
        ],
    );
    assert_eq!(recovered.status, ConditionStatus::True);
    assert_eq!(recovered.reason, "PolicyApplied");
}

#[test]
fn access_policy_condition_invalid_policy() {
    let mut bad = valid_grant("bad-grant");
    bad.rules[0].verbs = vec!["*".into()];
    let fact = access_policy_condition(&[bad], &[]);
    assert_eq!(fact.status, ConditionStatus::False);
    assert_eq!(fact.reason, "InvalidPolicy");
    assert!(fact.message.contains("bad-grant"));
}

#[test]
fn access_policy_condition_missing_outcome_is_not_applied() {
    let fact = access_policy_condition(&[valid_grant("grant-one")], &[]);
    assert_eq!(fact.status, ConditionStatus::False);
    assert_eq!(fact.reason, "ApplyFailed");
    assert!(fact.message.contains("app-ns/grant-one"));
}

#[test]
fn access_policy_condition_same_name_in_two_namespaces_is_keyed_by_namespace() {
    let grants = [
        AccessGrant::new(
            "app-ns",
            "grant-x",
            vec![sample_rule()],
            vec![sample_subject()],
        ),
        AccessGrant::new(
            "other-ns",
            "grant-x",
            vec![sample_rule()],
            vec![sample_subject()],
        ),
    ];
    let fact = access_policy_condition(
        &grants,
        &[
            GrantOutcome::applied("app-ns", "grant-x"),
            GrantOutcome::failed("other-ns", "grant-x"),
        ],
    );
    assert_eq!(fact.status, ConditionStatus::False);
    assert_eq!(fact.reason, "ApplyFailed");
    assert!(fact.message.contains("other-ns/grant-x"));
    assert!(!fact.message.contains("app-ns/grant-x"));
}
