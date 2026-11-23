use super::reconcile_metrics::CountedService;
use super::*;
use std::time::Duration;

#[test]
fn from_list_splits_trims_and_dedups() {
    assert_eq!(
        WatchScope::from_list(" a, b ,a,,c "),
        WatchScope::Namespaces(vec!["a".to_string(), "b".to_string(), "c".to_string()])
    );
}

#[test]
fn from_list_empty_is_all_namespaces() {
    assert_eq!(WatchScope::from_list(""), WatchScope::AllNamespaces);
    assert_eq!(WatchScope::from_list(" , "), WatchScope::AllNamespaces);
}

#[test]
fn from_env_unset_is_all_namespaces() {
    assert_eq!(
        WatchScope::from_env("SERVICE_K8S_TEST_WATCH_UNSET_635"),
        WatchScope::AllNamespaces
    );
}

#[tokio::test]
async fn listed_namespaces_use_namespaced_watch_only() {
    let (client, seen) = fake_apiserver(vec![]);
    let leadership = Leadership::held(Election::new("test-identity".to_string()));
    let metrics = Arc::new(ControllerMetrics::new(CountedService::MANAGER));
    let scope = WatchScope::Namespaces(vec!["team-a".to_string(), "team-b".to_string()]);

    // The watch never ends; the timeout only stops it after the first requests.
    let _ = tokio::time::timeout(
        Duration::from_millis(400),
        watch::<CountedService>(client, leadership, metrics, scope),
    )
    .await;

    let log = seen.lock().unwrap().clone();
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/namespaces/team-a/countedservices"),
        "expected a namespaced watch for team-a, got {log:?}"
    );
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/namespaces/team-b/countedservices"),
        "expected a namespaced watch for team-b, got {log:?}"
    );
    assert!(
        !log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/countedservices"),
        "no cluster-wide list or watch may be made, got {log:?}"
    );
    assert!(
        log.iter()
            .filter(|r| r.contains("/namespaces/"))
            .all(|r| r.contains("/namespaces/team-a/") || r.contains("/namespaces/team-b/")),
        "no request may name an unlisted namespace, got {log:?}"
    );
}

#[tokio::test]
async fn all_namespaces_keeps_the_cluster_wide_watch() {
    let (client, seen) = fake_apiserver(vec![]);
    let leadership = Leadership::held(Election::new("test-identity".to_string()));
    let metrics = Arc::new(ControllerMetrics::new(CountedService::MANAGER));

    let _ = tokio::time::timeout(
        Duration::from_millis(400),
        watch::<CountedService>(client, leadership, metrics, WatchScope::AllNamespaces),
    )
    .await;

    let log = seen.lock().unwrap().clone();
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/countedservices"),
        "expected the cluster-wide watch, got {log:?}"
    );
    assert!(
        !log.iter().any(|r| r.contains("/namespaces/")),
        "no namespaced request may be made, got {log:?}"
    );
}
