#![cfg(feature = "controller")]

use serde_json::{json, Value};

use service_k8s::service::ConditionStatus;
use service_k8s::trust_bundle::{StoreError, OWNER_IDENTITY_LABEL};
use service_k8s::{
    reconcile_access, render_access_grants, AccessGrant, AccessObjectKind, AccessOutcome,
    AccessReport, AccessRule, AccessSubject, InMemoryAccessStore,
};

const OWNER: &str = "svc-a";

fn rule(verbs: &[&str]) -> AccessRule {
    AccessRule::new(
        vec!["apps".into()],
        vec!["deployments".into()],
        vec!["my-app".into()],
        verbs.iter().map(|verb| verb.to_string()).collect(),
    )
}

fn grant(namespace: &str, name: &str) -> AccessGrant {
    AccessGrant::new(
        namespace,
        name,
        vec![rule(&["get", "update"])],
        vec![AccessSubject::new("other-ns", "app-sa")],
    )
}

/// The rendered Role and RoleBinding of `grant`, without an owner label.
fn rendered(grant: &AccessGrant) -> (Value, Value) {
    let objects = render_access_grants(std::slice::from_ref(grant));
    (objects[0].clone(), objects[1].clone())
}

fn with_owner(mut object: Value, owner: Option<&str>) -> Value {
    if let Some(owner) = owner {
        object["metadata"]["labels"][OWNER_IDENTITY_LABEL] = json!(owner);
    }
    object
}

/// The Role and RoleBinding that a pass owned by `owner` must write.
fn expected(grant: &AccessGrant, owner: &str) -> (Value, Value) {
    let (role, binding) = rendered(grant);
    (
        with_owner(role, Some(owner)),
        with_owner(binding, Some(owner)),
    )
}

/// Put a Role and a RoleBinding for `grant` in the store, with `label` as owner.
fn seed(store: &InMemoryAccessStore, grant: &AccessGrant, label: Option<&str>) {
    let (role, binding) = rendered(grant);
    store.insert(
        AccessObjectKind::Role,
        &grant.namespace,
        &grant.name,
        with_owner(role, label),
    );
    store.insert(
        AccessObjectKind::RoleBinding,
        &grant.namespace,
        &grant.name,
        with_owner(binding, label),
    );
}

fn owner_label(store: &InMemoryAccessStore, kind: AccessObjectKind, name: &str) -> Value {
    store.get(kind, "app-ns", name).expect("object exists")["metadata"]["labels"]
        [OWNER_IDENTITY_LABEL]
        .clone()
}

fn pair(namespace: &str, name: &str) -> (String, String) {
    (namespace.to_string(), name.to_string())
}

fn outcome<'a>(report: &'a AccessReport, namespace: &str, name: &str) -> &'a AccessOutcome {
    &report.outcomes[&pair(namespace, name)]
}

fn applies(store: &InMemoryAccessStore) -> usize {
    store
        .calls()
        .iter()
        .filter(|call| call.starts_with("apply"))
        .count()
}

#[tokio::test]
async fn first_pass_creates_role_and_binding_and_a_changed_rule_updates_them() {
    let store = InMemoryAccessStore::new();
    let g = grant("app-ns", "g1");

    let report = reconcile_access(&store, OWNER, std::slice::from_ref(&g), &[], false).await;

    assert_eq!(*outcome(&report, "app-ns", "g1"), AccessOutcome::Applied);
    let (role, binding) = expected(&g, OWNER);
    assert_eq!(
        store.get(AccessObjectKind::Role, "app-ns", "g1"),
        Some(role)
    );
    let stored_binding = store
        .get(AccessObjectKind::RoleBinding, "app-ns", "g1")
        .expect("binding stored");
    assert_eq!(stored_binding, binding);
    assert_eq!(stored_binding["subjects"][0]["namespace"], "other-ns");
    assert!(report.is_ready);
    assert_eq!(report.condition.reason, "PolicyApplied");

    let changed = AccessGrant::new("app-ns", "g1", vec![rule(&["get"])], g.subjects.clone());
    reconcile_access(&store, OWNER, std::slice::from_ref(&changed), &[], false).await;
    let stored_role = store
        .get(AccessObjectKind::Role, "app-ns", "g1")
        .expect("role stored");
    assert_eq!(stored_role, expected(&changed, OWNER).0);
    assert_eq!(stored_role["rules"][0]["verbs"], json!(["get"]));
}

#[tokio::test]
async fn a_role_owned_by_another_owner_is_not_written() {
    let store = InMemoryAccessStore::new();
    let g = grant("app-ns", "g1");
    seed(&store, &g, Some("svc-b"));

    let report = reconcile_access(&store, OWNER, std::slice::from_ref(&g), &[], false).await;

    assert_eq!(
        *outcome(&report, "app-ns", "g1"),
        AccessOutcome::OwnerConflict {
            foreign_owner: "svc-b".into()
        }
    );
    assert_eq!(applies(&store), 0);
    assert_eq!(
        owner_label(&store, AccessObjectKind::Role, "g1"),
        json!("svc-b")
    );
    assert!(!report.is_ready);
    assert_eq!(report.condition.status, ConditionStatus::False);
    assert_eq!(report.condition.reason, "ApplyFailed");
}

#[tokio::test]
async fn a_role_that_is_ours_with_a_foreign_binding_is_refused_with_no_write() {
    let store = InMemoryAccessStore::new();
    let g = grant("app-ns", "g1");
    let (role, binding) = rendered(&g);
    store.insert(
        AccessObjectKind::Role,
        "app-ns",
        "g1",
        with_owner(role, Some(OWNER)),
    );
    store.insert(
        AccessObjectKind::RoleBinding,
        "app-ns",
        "g1",
        with_owner(binding, Some("svc-b")),
    );
    let seeded_role = store.get(AccessObjectKind::Role, "app-ns", "g1");

    let report = reconcile_access(&store, OWNER, std::slice::from_ref(&g), &[], false).await;

    assert_eq!(
        *outcome(&report, "app-ns", "g1"),
        AccessOutcome::OwnerConflict {
            foreign_owner: "svc-b".into()
        }
    );
    assert_eq!(applies(&store), 0);
    assert_eq!(
        store.get(AccessObjectKind::Role, "app-ns", "g1"),
        seeded_role
    );
}

#[tokio::test]
async fn an_unowned_object_is_refused_then_adopted_when_adoption_is_on() {
    let store = InMemoryAccessStore::new();
    let g = grant("app-ns", "g1");
    seed(&store, &g, None);

    let refused = reconcile_access(&store, OWNER, std::slice::from_ref(&g), &[], false).await;
    assert_eq!(
        *outcome(&refused, "app-ns", "g1"),
        AccessOutcome::AdoptionRefusedUnowned
    );
    assert_eq!(applies(&store), 0);

    let adopted = reconcile_access(&store, OWNER, std::slice::from_ref(&g), &[], true).await;
    assert_eq!(*outcome(&adopted, "app-ns", "g1"), AccessOutcome::Adopted);
    assert!(adopted.is_ready);
    assert_eq!(
        owner_label(&store, AccessObjectKind::Role, "g1"),
        json!(OWNER)
    );
}

#[tokio::test]
async fn a_stale_owned_grant_is_pruned_but_foreign_and_unowned_ones_are_kept() {
    let store = InMemoryAccessStore::new();
    seed(&store, &grant("app-ns", "old"), Some(OWNER));
    seed(&store, &grant("app-ns", "theirs"), Some("svc-b"));
    seed(&store, &grant("app-ns", "loose"), None);
    let candidates = [
        pair("app-ns", "old"),
        pair("app-ns", "theirs"),
        pair("app-ns", "loose"),
    ];

    let report = reconcile_access(&store, OWNER, &[], &candidates, false).await;

    assert_eq!(*outcome(&report, "app-ns", "old"), AccessOutcome::Pruned);
    assert_eq!(store.get(AccessObjectKind::Role, "app-ns", "old"), None);
    assert_eq!(
        store.get(AccessObjectKind::RoleBinding, "app-ns", "old"),
        None
    );
    assert_eq!(
        *outcome(&report, "app-ns", "theirs"),
        AccessOutcome::ForeignRetained
    );
    assert_eq!(
        *outcome(&report, "app-ns", "loose"),
        AccessOutcome::ForeignRetained
    );
    assert!(store
        .get(AccessObjectKind::Role, "app-ns", "theirs")
        .is_some());
    assert!(store
        .get(AccessObjectKind::RoleBinding, "app-ns", "theirs")
        .is_some());
    assert!(store
        .get(AccessObjectKind::Role, "app-ns", "loose")
        .is_some());
    assert!(store
        .get(AccessObjectKind::RoleBinding, "app-ns", "loose")
        .is_some());
    let deletes = store
        .calls()
        .iter()
        .filter(|call| call.starts_with("delete"))
        .count();
    assert_eq!(deletes, 2);
}

#[tokio::test]
async fn deny_all_writes_nothing_and_is_ready() {
    let store = InMemoryAccessStore::new();

    let report = reconcile_access(&store, OWNER, &[], &[], false).await;

    assert!(store.calls().is_empty());
    assert!(report.is_ready);
    assert_eq!(report.condition.reason, "DenyAll");
    assert!(report.outcomes.is_empty());

    seed(&store, &grant("app-ns", "old"), Some(OWNER));
    let pruned = reconcile_access(&store, OWNER, &[], &[pair("app-ns", "old")], false).await;

    assert_eq!(*outcome(&pruned, "app-ns", "old"), AccessOutcome::Pruned);
    assert!(pruned.is_ready);
    assert_eq!(pruned.condition.reason, "DenyAll");
}

#[tokio::test]
async fn a_failing_namespace_isolates_and_the_next_pass_recovers() {
    let store = InMemoryAccessStore::new();
    let grants = [grant("ns-a", "g"), grant("ns-b", "g")];
    store.fail_namespace("ns-b", StoreError::unavailable("down"));

    let report = reconcile_access(&store, OWNER, &grants, &[], false).await;

    assert_eq!(*outcome(&report, "ns-a", "g"), AccessOutcome::Applied);
    assert!(store.get(AccessObjectKind::Role, "ns-a", "g").is_some());
    assert!(matches!(
        outcome(&report, "ns-b", "g"),
        AccessOutcome::Failed { .. }
    ));
    assert!(!report.is_ready);
    assert_eq!(report.condition.reason, "ApplyFailed");
    assert_eq!(store.get(AccessObjectKind::Role, "ns-b", "g"), None);

    store.clear_namespace_failure("ns-b");
    let recovered = reconcile_access(&store, OWNER, &grants, &[], false).await;

    assert_eq!(*outcome(&recovered, "ns-a", "g"), AccessOutcome::Applied);
    assert_eq!(*outcome(&recovered, "ns-b", "g"), AccessOutcome::Applied);
    assert!(recovered.is_ready);
    assert_eq!(recovered.condition.reason, "PolicyApplied");
}

#[tokio::test]
async fn an_invalid_policy_makes_zero_store_calls() {
    let store = InMemoryAccessStore::new();
    seed(&store, &grant("app-ns", "old"), Some(OWNER));
    let invalid = AccessGrant::new(
        "app-ns",
        "g1",
        vec![rule(&["*"])],
        vec![AccessSubject::new("other-ns", "app-sa")],
    );

    let report = reconcile_access(
        &store,
        OWNER,
        std::slice::from_ref(&invalid),
        &[pair("app-ns", "old")],
        false,
    )
    .await;

    assert!(store.calls().is_empty());
    assert!(report.outcomes.is_empty());
    assert!(!report.is_ready);
    assert_eq!(report.condition.reason, "InvalidPolicy");
}

#[tokio::test]
async fn the_store_only_ever_receives_roles_and_role_bindings() {
    let store = InMemoryAccessStore::new();
    seed(&store, &grant("app-ns", "old"), Some(OWNER));
    let grants = [grant("app-ns", "g1"), grant("app-ns", "g2")];

    reconcile_access(&store, OWNER, &grants, &[pair("app-ns", "old")], false).await;

    let calls = store.calls();
    assert!(!calls.is_empty());
    for call in &calls {
        assert!(
            call.contains(" Role ") || call.contains(" RoleBinding "),
            "unexpected kind in call: {call}"
        );
        if call.starts_with("apply") {
            assert!(
                call.ends_with("kind=Role") || call.ends_with("kind=RoleBinding"),
                "apply sent an unexpected object: {call}"
            );
        }
        for forbidden in ["ClusterRole", "Namespace", "ServiceAccount", "Secret"] {
            assert!(!call.contains(forbidden), "forbidden kind in call: {call}");
        }
    }
}
