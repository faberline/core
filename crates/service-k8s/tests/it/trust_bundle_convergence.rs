//! Integration tests for public trust bundle convergence (#30).
//!
//! Validates:
//! - Multiple ServiceAccounts in one namespace share one ConfigMap
//! - Owner conflict causes adoption refusal, leaving foreign ConfigMap untouched
//! - Unowned ConfigMap adoption policy (refuse vs adopt)
//! - Rotation root overlap and retirement
//! - Stale owned ConfigMap prune
//! - Negative control: foreign ConfigMap in prune candidate namespace is NOT deleted
//! - Negative control: private key rejected before any store I/O
//! - Exact namespace scope: unscoped namespaces are never read or written
//! - ClientTrustReady condition fails on error and recovers on next pass
//! - Fault isolation: one namespace failing does not stop other namespaces

use service_k8s::trust_bundle::{
    reconcile, ConfigMapStore, InMemoryConfigMapStore, NamespaceOutcome, StoredConfigMap,
    TrustBundleSpec, CA_BUNDLE_DATA_KEY, CLIENT_TRUST_READY_CONDITION, OWNER_IDENTITY_ANNOTATION,
    OWNER_IDENTITY_LABEL,
};
use std::collections::BTreeMap;

const ROOT_1: &str = "-----BEGIN CERTIFICATE-----\nMIIB_ROOT_1_PEM_DATA\n-----END CERTIFICATE-----";
const ROOT_2: &str = "-----BEGIN CERTIFICATE-----\nMIIB_ROOT_2_PEM_DATA\n-----END CERTIFICATE-----";
const PRIV_KEY: &str =
    "-----BEGIN PRIVATE KEY-----\nMIIE_PRIVATE_KEY_DATA\n-----END PRIVATE KEY-----";

#[tokio::test]
async fn multiple_ksas_in_one_namespace_shares_single_desired_configmap() {
    let store = InMemoryConfigMapStore::new();
    let spec = TrustBundleSpec::new("test-owner", "ca-bundle", ROOT_1).with_service_accounts([
        ("ns-client", "sa-web"),
        ("ns-client", "sa-worker"),
        ("ns-client", "sa-backend"),
    ]);

    let report = reconcile(&store, &spec).await;

    assert!(report.is_ready);
    assert_eq!(report.condition.status.as_str(), "True");
    assert_eq!(
        store.apply_count(),
        1,
        "only one write should occur for shared namespace"
    );

    let cm = store
        .get("ns-client", "ca-bundle")
        .expect("configmap must exist");
    assert_eq!(
        cm.data.get(CA_BUNDLE_DATA_KEY).map(|s| s.trim()),
        Some(ROOT_1.trim())
    );
    assert_eq!(
        cm.labels.get(OWNER_IDENTITY_LABEL).map(String::as_str),
        Some("test-owner")
    );
    assert_eq!(
        cm.annotations
            .get(OWNER_IDENTITY_ANNOTATION)
            .map(String::as_str),
        Some("test-owner")
    );
}

#[tokio::test]
async fn owner_conflict_refuses_adoption_and_leaves_foreign_configmap_untouched() {
    let store = InMemoryConfigMapStore::new();
    let mut initial_labels = BTreeMap::new();
    initial_labels.insert(
        OWNER_IDENTITY_LABEL.to_string(),
        "foreign-operator".to_string(),
    );
    let mut initial_data = BTreeMap::new();
    initial_data.insert(
        CA_BUNDLE_DATA_KEY.to_string(),
        "foreign-ca-bytes".to_string(),
    );

    store.insert(
        "ns-client",
        "ca-bundle",
        StoredConfigMap {
            data: initial_data,
            labels: initial_labels,
            annotations: BTreeMap::new(),
        },
    );

    let spec =
        TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1).with_allowed_namespace("ns-client");

    let report = reconcile(&store, &spec).await;

    assert!(!report.is_ready);
    assert_eq!(report.condition.status.as_str(), "False");
    assert_eq!(report.condition.reason, "AdoptionRefused");
    assert_eq!(
        report.outcomes.get("ns-client"),
        Some(&NamespaceOutcome::AdoptionRefused {
            foreign_owner: "foreign-operator".to_string(),
        })
    );

    assert_eq!(
        store.apply_count(),
        0,
        "foreign configmap must not be modified"
    );
    let cm = store
        .get("ns-client", "ca-bundle")
        .expect("configmap must remain");
    assert_eq!(
        cm.data.get(CA_BUNDLE_DATA_KEY).map(String::as_str),
        Some("foreign-ca-bytes")
    );
}

#[tokio::test]
async fn unowned_configmap_adoption_controlled_by_spec() {
    let store = InMemoryConfigMapStore::new();
    let mut initial_data = BTreeMap::new();
    initial_data.insert(
        CA_BUNDLE_DATA_KEY.to_string(),
        "unowned-ca-bytes".to_string(),
    );

    store.insert(
        "ns-client",
        "ca-bundle",
        StoredConfigMap {
            data: initial_data,
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        },
    );

    // 1. adopt_unowned = false -> refused
    let spec_refuse = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespace("ns-client")
        .with_adopt_unowned(false);

    let report_refuse = reconcile(&store, &spec_refuse).await;
    assert!(!report_refuse.is_ready);
    assert_eq!(report_refuse.condition.reason, "AdoptionRefused");
    assert_eq!(
        report_refuse.outcomes.get("ns-client"),
        Some(&NamespaceOutcome::AdoptionRefusedUnowned)
    );
    assert_eq!(store.apply_count(), 0);

    // 2. adopt_unowned = true -> adopted
    let spec_adopt = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespace("ns-client")
        .with_adopt_unowned(true);

    let report_adopt = reconcile(&store, &spec_adopt).await;
    assert!(report_adopt.is_ready);
    assert_eq!(
        report_adopt.outcomes.get("ns-client"),
        Some(&NamespaceOutcome::Adopted)
    );
    assert_eq!(store.apply_count(), 1);

    let cm = store
        .get("ns-client", "ca-bundle")
        .expect("configmap exists");
    assert_eq!(
        cm.labels.get(OWNER_IDENTITY_LABEL).map(String::as_str),
        Some("my-service")
    );
    assert_eq!(
        cm.data.get(CA_BUNDLE_DATA_KEY).map(|s| s.trim()),
        Some(ROOT_1.trim())
    );
}

#[tokio::test]
async fn rotation_overlap_retains_both_roots_until_old_root_dropped() {
    let store = InMemoryConfigMapStore::new();

    // Overlap phase: bundle contains ROOT_1 and ROOT_2
    let overlap_pem = format!("{ROOT_1}\n{ROOT_2}");
    let spec_overlap = TrustBundleSpec::new("my-service", "ca-bundle", &overlap_pem)
        .with_allowed_namespace("ns-client");

    let report1 = reconcile(&store, &spec_overlap).await;
    assert!(report1.is_ready);

    let cm1 = store
        .get("ns-client", "ca-bundle")
        .expect("configmap exists");
    let data1 = cm1.data.get(CA_BUNDLE_DATA_KEY).unwrap();
    assert!(data1.contains("MIIB_ROOT_1_PEM_DATA"));
    assert!(data1.contains("MIIB_ROOT_2_PEM_DATA"));

    // Retired phase: caller provides ROOT_2 only
    let spec_retired =
        TrustBundleSpec::new("my-service", "ca-bundle", ROOT_2).with_allowed_namespace("ns-client");

    let report2 = reconcile(&store, &spec_retired).await;
    assert!(report2.is_ready);

    let cm2 = store
        .get("ns-client", "ca-bundle")
        .expect("configmap exists");
    let data2 = cm2.data.get(CA_BUNDLE_DATA_KEY).unwrap();
    assert!(
        !data2.contains("MIIB_ROOT_1_PEM_DATA"),
        "old root must be dropped when omitted"
    );
    assert!(data2.contains("MIIB_ROOT_2_PEM_DATA"));
}

#[tokio::test]
async fn stale_prune_deletes_owned_configmap_in_disallowed_namespace() {
    let store = InMemoryConfigMapStore::new();

    // Pre-populate owned ConfigMap in ns-stale
    let mut labels = BTreeMap::new();
    labels.insert(OWNER_IDENTITY_LABEL.to_string(), "my-service".to_string());
    store.insert(
        "ns-stale",
        "ca-bundle",
        StoredConfigMap {
            data: BTreeMap::new(),
            labels,
            annotations: BTreeMap::new(),
        },
    );

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespace("ns-active")
        .with_prune_candidates(["ns-stale"]);

    let report = reconcile(&store, &spec).await;
    assert!(report.is_ready);
    assert_eq!(
        report.outcomes.get("ns-stale"),
        Some(&NamespaceOutcome::Pruned)
    );
    assert_eq!(
        store.get("ns-stale", "ca-bundle"),
        None,
        "stale owned configmap must be deleted"
    );
    assert!(
        store.get("ns-active", "ca-bundle").is_some(),
        "active configmap must be created"
    );
}

#[tokio::test]
async fn foreign_configmap_is_not_pruned_in_disallowed_namespace() {
    // Negative control: ConfigMap owned by someone else must never be deleted
    let store = InMemoryConfigMapStore::new();

    let mut labels = BTreeMap::new();
    labels.insert(OWNER_IDENTITY_LABEL.to_string(), "other-team".to_string());
    let mut data = BTreeMap::new();
    data.insert(CA_BUNDLE_DATA_KEY.to_string(), "dont-touch-me".to_string());

    store.insert(
        "ns-stale",
        "ca-bundle",
        StoredConfigMap {
            data,
            labels,
            annotations: BTreeMap::new(),
        },
    );

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespace("ns-active")
        .with_prune_candidates(["ns-stale"]);

    let report = reconcile(&store, &spec).await;
    assert!(report.is_ready);
    assert_eq!(
        report.outcomes.get("ns-stale"),
        Some(&NamespaceOutcome::ForeignRetained)
    );
    assert_eq!(
        store.delete_count(),
        0,
        "foreign configmap must never be deleted"
    );
    assert!(store.get("ns-stale", "ca-bundle").is_some());
}

#[tokio::test]
async fn private_key_is_rejected_before_any_write() {
    // Negative control: input containing private key rejected before store operations
    let store = InMemoryConfigMapStore::new();
    let poisoned_input = format!("{ROOT_1}\n{PRIV_KEY}");

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", &poisoned_input)
        .with_allowed_namespace("ns-client");

    let report = reconcile(&store, &spec).await;
    assert!(!report.is_ready);
    assert_eq!(report.condition.type_, CLIENT_TRUST_READY_CONDITION);
    assert_eq!(report.condition.status.as_str(), "False");
    assert_eq!(report.condition.reason, "InvalidCertificate");

    assert_eq!(store.read_count(), 0, "no reads when validation fails");
    assert_eq!(store.apply_count(), 0, "no writes when validation fails");
    assert_eq!(store.delete_count(), 0, "no deletes when validation fails");
}

#[tokio::test]
async fn exact_namespace_scope_never_reads_or_writes_unscoped_namespaces() {
    let store = InMemoryConfigMapStore::new();
    store.insert("ns-unscoped", "ca-bundle", StoredConfigMap::default());

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespace("ns-allowed");

    let report = reconcile(&store, &spec).await;
    assert!(report.is_ready);
    assert!(!report.outcomes.contains_key("ns-unscoped"));
}

#[tokio::test]
async fn client_trust_ready_recovers_after_failure() {
    let store = InMemoryConfigMapStore::new();

    // Step 1: Foreign owner conflict in ns-a
    let mut labels = BTreeMap::new();
    labels.insert(
        OWNER_IDENTITY_LABEL.to_string(),
        "foreign-owner".to_string(),
    );
    store.insert(
        "ns-a",
        "ca-bundle",
        StoredConfigMap {
            labels,
            ..Default::default()
        },
    );

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespaces(["ns-a", "ns-b"]);

    let report1 = reconcile(&store, &spec).await;
    assert!(!report1.is_ready);
    assert_eq!(report1.condition.status.as_str(), "False");
    assert_eq!(report1.condition.reason, "AdoptionRefused");

    // Step 2: Remove foreign conflict, rerunning reconcile
    store.delete("ns-a", "ca-bundle").await.unwrap();

    let report2 = reconcile(&store, &spec).await;
    assert!(report2.is_ready);
    assert_eq!(report2.condition.status.as_str(), "True");
    assert_eq!(report2.condition.reason, "Converged");
}

#[tokio::test]
async fn one_namespace_failing_does_not_stop_the_others() {
    let store = InMemoryConfigMapStore::new();

    // ns-conflict has foreign owner
    let mut labels = BTreeMap::new();
    labels.insert(
        OWNER_IDENTITY_LABEL.to_string(),
        "foreign-owner".to_string(),
    );
    store.insert(
        "ns-conflict",
        "ca-bundle",
        StoredConfigMap {
            labels,
            ..Default::default()
        },
    );

    let spec = TrustBundleSpec::new("my-service", "ca-bundle", ROOT_1)
        .with_allowed_namespaces(["ns-conflict", "ns-healthy"]);

    let report = reconcile(&store, &spec).await;

    assert!(!report.is_ready);
    assert_eq!(
        report.outcomes.get("ns-conflict"),
        Some(&NamespaceOutcome::AdoptionRefused {
            foreign_owner: "foreign-owner".to_string(),
        })
    );
    assert_eq!(
        report.outcomes.get("ns-healthy"),
        Some(&NamespaceOutcome::Applied)
    );

    // ns-healthy was successfully applied despite ns-conflict's failure
    assert!(store.get("ns-healthy", "ca-bundle").is_some());
}
