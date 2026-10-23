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
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::Wildcard { grant, path }) if grant == "app-access" && path == "rules/0/verbs/0")
    );
}

#[test]
fn wildcard_in_resources_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resources = vec!["*".into()];
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::Wildcard { grant, path }) if grant == "app-access" && path == "rules/0/resources/0")
    );
}

#[test]
fn wildcard_in_api_groups_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].api_groups = vec!["*".into()];
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::Wildcard { grant, path }) if grant == "app-access" && path == "rules/0/apiGroups/0")
    );
}

#[test]
fn wildcard_in_resource_names_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resource_names = vec!["*".into()];
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::Wildcard { grant, path }) if grant == "app-access" && path == "rules/0/resourceNames/0")
    );
}

#[test]
fn wildcard_in_subject_name_is_refused() {
    let mut grant = valid_grant("app-access");
    grant.subjects[0].name = "*".into();
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::Wildcard { grant, path }) if grant == "app-access" && path == "subjects/0/name")
    );
}

#[test]
fn empty_resource_names_refused() {
    let mut grant = valid_grant("app-access");
    grant.rules[0].resource_names = vec![];
    assert!(
        matches!(validate_policy(&[grant]), Err(AccessGrantError::EmptyResourceNames { grant }) if grant == "app-access")
    );
}

#[test]
fn exact_subjects_and_rules_render_byte_for_byte() {
    let grant = AccessGrant::new(
        "billing-ns",
        "ledger-access",
        vec![AccessRule::new(
            vec!["accounting.example.com".into()],
            vec!["ledgers".into()],
            vec!["q4-report".into()],
            vec!["get".into(), "patch".into()],
        )],
        vec![
            AccessSubject::new("billing-ns", "local-worker"),
            AccessSubject::new("audit-ns", "external-auditor"),
        ],
    );

    let rendered = render_access_grants(&[grant]);
    assert_eq!(rendered.len(), 2);

    let expected_role = json!({
        "apiVersion": "rbac.authorization.k8s.io/v1",
        "kind": "Role",
        "metadata": {
            "name": "ledger-access",
            "namespace": "billing-ns",
            "labels": {}
        },
        "rules": [
            {
                "apiGroups": ["accounting.example.com"],
                "resources": ["ledgers"],
                "resourceNames": ["q4-report"],
                "verbs": ["get", "patch"]
            }
        ]
    });

    let expected_binding = json!({
        "apiVersion": "rbac.authorization.k8s.io/v1",
        "kind": "RoleBinding",
        "metadata": {
            "name": "ledger-access",
            "namespace": "billing-ns",
            "labels": {}
        },
        "roleRef": {
            "apiGroup": "rbac.authorization.k8s.io",
            "kind": "Role",
            "name": "ledger-access"
        },
        "subjects": [
            {
                "kind": "ServiceAccount",
                "name": "local-worker",
                "namespace": "billing-ns"
            },
            {
                "kind": "ServiceAccount",
                "name": "external-auditor",
                "namespace": "audit-ns"
            }
        ]
    });

    assert_eq!(rendered[0], expected_role);
    assert_eq!(rendered[1], expected_binding);
}

#[test]
fn render_order_is_stable_under_shuffled_input() {
    let g1 = AccessGrant::new(
        "beta-ns",
        "grant-b",
        vec![sample_rule()],
        vec![sample_subject()],
    );
    let g2 = AccessGrant::new(
        "alpha-ns",
        "grant-2",
        vec![sample_rule()],
        vec![sample_subject()],
    );
    let g3 = AccessGrant::new(
        "alpha-ns",
        "grant-1",
        vec![sample_rule()],
        vec![sample_subject()],
    );

    let list1 = vec![g1.clone(), g2.clone(), g3.clone()];
    let list2 = vec![g3.clone(), g1.clone(), g2.clone()];
    let list3 = vec![g2.clone(), g3.clone(), g1.clone()];

    let r1 = render_access_grants(&list1);
    let r2 = render_access_grants(&list2);
    let r3 = render_access_grants(&list3);

    assert_eq!(r1, r2);
    assert_eq!(r1, r3);

    assert_eq!(r1[0]["metadata"]["namespace"], "alpha-ns");
    assert_eq!(r1[0]["metadata"]["name"], "grant-1");
    assert_eq!(r1[2]["metadata"]["namespace"], "alpha-ns");
    assert_eq!(r1[2]["metadata"]["name"], "grant-2");
    assert_eq!(r1[4]["metadata"]["namespace"], "beta-ns");
    assert_eq!(r1[4]["metadata"]["name"], "grant-b");
}

#[test]
fn empty_set_is_deny_all_and_passes() {
    assert_eq!(validate_policy(&[]), Ok(()));
    let rendered = render_access_grants(&[]);
    assert!(rendered.is_empty());

    let fact = access_policy_condition(&[], &[]);
    assert_eq!(fact.type_, "AccessPolicyReady");
    assert_eq!(fact.status, ConditionStatus::True);
    assert_eq!(fact.reason, "DenyAll");
}

#[test]
fn duplicate_grant_refused() {
    let g1 = valid_grant("dup-grant");
    let g2 = valid_grant("dup-grant");
    assert!(
        matches!(validate_policy(&[g1, g2]), Err(AccessGrantError::DuplicateGrant { grant, namespace }) if grant == "dup-grant" && namespace == "app-ns")
    );
}

#[test]
fn access_policy_ready_condition_lifecycle_and_partial_failure() {
    let g1 = valid_grant("grant-one");
    let g2 = valid_grant("grant-two");
    let grants = vec![g1, g2];

    let partial_outcomes = vec![
        GrantOutcome::applied("grant-one"),
        GrantOutcome::failed("grant-two"),
    ];
    let fact = access_policy_condition(&grants, &partial_outcomes);
    assert_eq!(fact.type_, "AccessPolicyReady");
    assert_eq!(fact.status, ConditionStatus::False);
    assert_eq!(fact.reason, "ApplyFailed");
    assert!(
        fact.message.contains("grant-two"),
        "failed grant must be named in message"
    );
    assert!(
        !fact.message.contains("grant-one"),
        "succeeded grant must NOT be named in message"
    );

    let recovered_outcomes = vec![
        GrantOutcome::applied("grant-one"),
        GrantOutcome::applied("grant-two"),
    ];
    let recovered_fact = access_policy_condition(&grants, &recovered_outcomes);
    assert_eq!(recovered_fact.type_, "AccessPolicyReady");
    assert_eq!(recovered_fact.status, ConditionStatus::True);
    assert_eq!(recovered_fact.reason, "PolicyApplied");
}

#[test]
fn access_policy_condition_invalid_policy() {
    let mut grant = valid_grant("bad-grant");
    grant.rules[0].verbs = vec!["*".into()];
    let fact = access_policy_condition(&[grant], &[]);
    assert_eq!(fact.type_, "AccessPolicyReady");
    assert_eq!(fact.status, ConditionStatus::False);
    assert_eq!(fact.reason, "InvalidPolicy");
    assert!(fact.message.contains("bad-grant"));
}
