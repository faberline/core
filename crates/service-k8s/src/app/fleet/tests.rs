//! Tests of the Fleet controller loop (#660 child, issue #679). A route-based
//! fake apiserver records every request. The adapter calls `Handle::block_on`
//! inside `spawn_blocking`, so the pass tests run on a multi-thread runtime.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kube::client::Body;
use kube::runtime::controller::Action;
use kube::Client;
use serde_json::{json, Value};

use super::*;
use crate::api::fleet::{FleetInstance, FleetSpec, FleetStatus};
use crate::domain::condition::Condition;
use crate::domain::fleet::{FleetEntryPhase, FleetEntryStatus, FleetTarget};
use crate::domain::leadership::Election;
use crate::interfaces::operator::WatchScope;

const CHILD_BASE: &str = "/apis/apps/v1/namespaces/tenant-a/statefulsets";
const FLEET_STATUS_PATH: &str =
    "/apis/service-k8s.test/v1/namespaces/tenant-a/testfleets/fleet-1/status";

#[derive(
    kube::CustomResource, Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "TestFleet",
    namespaced,
    status = "FleetStatus"
)]
pub struct TestFleetSpec {
    #[serde(flatten)]
    fleet: FleetSpec<Value>,
}

impl FleetService for TestFleet {
    type Child = Value;
    const MANAGER: &'static str = "faberline-fleet";
    const CHILD_API_VERSION: &'static str = "apps/v1";
    const CHILD_KIND: &'static str = "StatefulSet";

    fn fleet_spec(&self) -> &FleetSpec<Self::Child> {
        &self.spec.fleet
    }

    fn fleet_conditions(&self) -> &[Condition] {
        self.status
            .as_ref()
            .map(|status| status.conditions.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    body: Value,
}

type Seen = Arc<Mutex<Vec<Recorded>>>;

/// A route-based fake apiserver. `route` gets the method and the path and
/// returns a status code and a JSON body.
fn fake(route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static) -> (Client, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let log = log.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = route(&method, &path);
            log.lock().unwrap().push(Recorded { method, path, body });
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "default"), seen)
}

/// The recorded requests as `"{method} {path}"`.
fn log(seen: &Seen) -> Vec<String> {
    seen.lock()
        .unwrap()
        .iter()
        .map(|r| format!("{} {}", r.method, r.path))
        .collect()
}

/// The body of the last request to the Fleet status subresource.
fn last_status_body(seen: &Seen) -> Value {
    seen.lock()
        .unwrap()
        .iter()
        .rev()
        .find(|r| r.method == "PATCH" && r.path == FLEET_STATUS_PATH)
        .map(|r| r.body.clone())
        .expect("a status patch was recorded")
}

fn not_found() -> Value {
    json!({"kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404})
}

fn internal(message: &str) -> Value {
    json!({"kind": "Status", "status": "Failure", "reason": "InternalError", "code": 500, "message": message})
}

fn statefulset(name: &str) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "StatefulSet",
        "metadata": {"name": name, "namespace": "tenant-a"},
        "spec": {"replicas": 1},
    })
}

fn statefulset_list() -> Value {
    json!({"apiVersion": "apps/v1", "kind": "StatefulSetList", "metadata": {}, "items": []})
}

fn fleet_body() -> Value {
    json!({
        "apiVersion": "service-k8s.test/v1",
        "kind": "TestFleet",
        "metadata": {"name": "fleet-1", "namespace": "tenant-a"},
        "spec": {"instances": []},
    })
}

fn leader_election() -> Arc<Election> {
    let election = Election::new("test-identity".to_string());
    election.is_leader.store(true, Ordering::Relaxed);
    election
}

fn instance(target: FleetTarget, name: &str) -> FleetInstance<Value> {
    FleetInstance {
        target,
        name: Some(name.to_string()),
        spec: json!({"replicas": 1}),
    }
}

/// A Fleet object in `tenant-a` with uid `uid-1`, generation 1 and name `fleet-1`.
fn fleet_obj(instances: Vec<FleetInstance<Value>>, max_concurrent: u32) -> Arc<TestFleet> {
    let fleet: FleetSpec<Value> = serde_json::from_value(json!({
        "instances": serde_json::to_value(&instances).unwrap(),
        "rollout": {"maxConcurrent": max_concurrent},
    }))
    .unwrap();
    let mut obj = TestFleet::new("fleet-1", TestFleetSpec { fleet });
    obj.metadata.namespace = Some("tenant-a".to_string());
    obj.metadata.uid = Some("uid-1".to_string());
    obj.metadata.generation = Some(1);
    Arc::new(obj)
}

fn entry(phase: FleetEntryPhase) -> FleetEntryStatus {
    FleetEntryStatus {
        target: FleetTarget::local("tenant-a"),
        phase,
        observed_hash: None,
        message: None,
    }
}

#[test]
fn run_fleet_is_exported_from_controller() {
    // Names the public path `service_k8s::controller::run_fleet` and the rest of the surface.
    let _ = crate::controller::run_fleet::<TestFleet>;
    let _ = crate::controller::reconcile_fleet_object::<TestFleet>;
    let _ = crate::controller::FLEET_REPAIR_REQUEUE;
    let _: Option<crate::controller::FleetError> = None;
}

#[tokio::test]
async fn listed_namespaces_use_namespaced_watch_only() {
    let (client, seen) = fake(|_, _| (500, internal("no watch")));
    let scope = WatchScope::Namespaces(vec!["team-a".to_string(), "team-b".to_string()]);

    // The watch never ends; the timeout only stops it after the first requests.
    let _ = tokio::time::timeout(
        Duration::from_millis(400),
        watch_fleet::<TestFleet>(client, leader_election(), scope),
    )
    .await;

    let log = log(&seen);
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/namespaces/team-a/testfleets"),
        "expected a namespaced watch for team-a, got {log:?}"
    );
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/namespaces/team-b/testfleets"),
        "expected a namespaced watch for team-b, got {log:?}"
    );
    assert!(
        !log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/testfleets"),
        "no cluster-wide list or watch may be made, got {log:?}"
    );
}

#[tokio::test]
async fn all_namespaces_keeps_the_cluster_wide_watch() {
    let (client, seen) = fake(|_, _| (500, internal("no watch")));

    let _ = tokio::time::timeout(
        Duration::from_millis(400),
        watch_fleet::<TestFleet>(client, leader_election(), WatchScope::AllNamespaces),
    )
    .await;

    let log = log(&seen);
    assert!(
        log.iter()
            .any(|r| r == "GET /apis/service-k8s.test/v1/testfleets"),
        "expected the cluster-wide watch, got {log:?}"
    );
    assert!(
        !log.iter().any(|r| r.contains("/namespaces/")),
        "no namespaced request may be made, got {log:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_api_failure_of_one_entry_marks_it_failed_and_the_status_is_still_patched() {
    let (client, seen) = fake(|method, path| match (method, path) {
        ("GET", p) if p == format!("{CHILD_BASE}/a") || p == format!("{CHILD_BASE}/b") => {
            (404, not_found())
        }
        ("GET", CHILD_BASE) => (200, statefulset_list()),
        ("PATCH", p) if p == format!("{CHILD_BASE}/a") => (200, statefulset("a")),
        ("PATCH", p) if p == format!("{CHILD_BASE}/b") => (500, internal("boom")),
        ("PATCH", FLEET_STATUS_PATH) => (200, fleet_body()),
        _ => (404, not_found()),
    });
    let obj = fleet_obj(
        vec![
            instance(FleetTarget::local("tenant-a"), "a"),
            instance(FleetTarget::local("tenant-a"), "b"),
        ],
        2,
    );
    let scope = WatchScope::Namespaces(vec!["tenant-a".to_string()]);

    let action = reconcile_fleet_object(client, obj, leader_election(), &scope)
        .await
        .expect("the pass completes and writes its status");

    assert_eq!(action, Action::requeue(FLEET_PROGRESS_REQUEUE));
    let requests = log(&seen);
    assert!(
        !requests
            .iter()
            .any(|r| r == "GET /apis/apps/v1/statefulsets"),
        "no cluster-wide list of children may be made, got {requests:?}"
    );
    let status = last_status_body(&seen);
    assert_eq!(status["status"]["entries"][0]["phase"], "Applying");
    assert_eq!(status["status"]["entries"][1]["phase"], "Failed");
    assert!(
        status["status"]["entries"][1]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("boom"),
        "the failed entry must carry the apiserver message, got {status}"
    );
}

#[tokio::test]
async fn a_follower_makes_no_request() {
    let (client, seen) = fake(|_, _| (500, internal("must not be called")));
    let election = Election::new("follower".to_string());
    let obj = fleet_obj(vec![instance(FleetTarget::local("tenant-a"), "a")], 1);

    let action = reconcile_fleet_object(client, obj, election, &WatchScope::AllNamespaces)
        .await
        .expect("a follower answers with a requeue");

    assert_eq!(action, Action::requeue(Duration::from_secs(10)));
    assert!(
        log(&seen).is_empty(),
        "a follower made requests: {:?}",
        log(&seen)
    );
}

#[test]
fn requeue_after_is_the_repair_time_only_when_every_entry_is_ready() {
    assert_eq!(FLEET_REPAIR_REQUEUE, Duration::from_secs(300));
    assert_eq!(FLEET_PROGRESS_REQUEUE, Duration::from_secs(15));

    let ready = FleetStatus {
        entries: vec![entry(FleetEntryPhase::Ready)],
        conditions: vec![],
    };
    assert_eq!(requeue_after(&ready), FLEET_REPAIR_REQUEUE);

    let applying = FleetStatus {
        entries: vec![entry(FleetEntryPhase::Applying)],
        conditions: vec![],
    };
    assert_eq!(requeue_after(&applying), FLEET_PROGRESS_REQUEUE);

    let empty = FleetStatus::default();
    assert_eq!(requeue_after(&empty), FLEET_PROGRESS_REQUEUE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_cluster_entries_are_reported_and_nothing_is_applied() {
    // Only the status subresource answers; any child request fails the test.
    let (client, seen) = fake(|method, path| match (method, path) {
        ("PATCH", FLEET_STATUS_PATH) => (200, fleet_body()),
        _ => (500, internal("must not be called")),
    });
    let obj = fleet_obj(
        vec![instance(
            FleetTarget::new(Some("east".to_string()), "tenant-a"),
            "a",
        )],
        1,
    );
    let scope = WatchScope::Namespaces(vec!["tenant-a".to_string()]);

    let action = reconcile_fleet_object(client, obj, leader_election(), &scope)
        .await
        .expect("the remote entry is reported, not applied");

    assert_eq!(action, Action::requeue(FLEET_REPAIR_REQUEUE));
    assert_eq!(
        log(&seen),
        vec![format!("PATCH {FLEET_STATUS_PATH}")],
        "only the status patch may be made"
    );
    let status = last_status_body(&seen);
    assert_eq!(status["status"]["entries"][0]["phase"], "Unreachable");
    assert!(
        status["status"]["entries"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("east"),
        "the message must name the remote cluster, got {status}"
    );
}

#[test]
fn unsupported_cluster_status_is_none_when_every_entry_is_local() {
    let local: FleetSpec<Value> = serde_json::from_value(json!({
        "instances": [{"target": {"namespace": "tenant-a"}, "spec": {}}],
    }))
    .unwrap();
    assert_eq!(unsupported_cluster_status(&local, &[], 1, "now"), None);
}

#[tokio::test]
async fn a_fleet_without_a_uid_is_an_error() {
    let (client, seen) = fake(|_, _| (500, internal("must not be called")));
    let mut obj = (*fleet_obj(vec![instance(FleetTarget::local("tenant-a"), "a")], 1)).clone();
    obj.metadata.uid = None;

    let result = reconcile_fleet_object(
        client,
        Arc::new(obj),
        leader_election(),
        &WatchScope::AllNamespaces,
    )
    .await;

    assert!(
        matches!(result, Err(FleetError::MissingUid(_))),
        "expected MissingUid, got {result:?}"
    );
    assert!(
        log(&seen).is_empty(),
        "no request may be made: {:?}",
        log(&seen)
    );
}
