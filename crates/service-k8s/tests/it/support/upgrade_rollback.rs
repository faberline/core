//! Shared harness for the upgrade-rollback tests (#372). It applies a render
//! through the current build's `reconcile_once`, against a client the caller
//! supplies. The render is either built here (`current_render`) or read from
//! the old render file that ci/upgrade-rollback-old-render.sh writes.

#![cfg(feature = "controller")]
#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use kube::client::Body;
use kube::{Client, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use service_k8s::controller::{reconcile_once, Error};
use service_k8s::render::{client_service, service_account, RenderCtx};
use service_k8s::service::{Condition, ManagedService, ReadinessTarget, ReadyFacts};
use service_k8s::Election;

pub const NAMESPACE: &str = "upgrade-rollback";
pub const OLD_RENDER_ENV: &str = "UPGRADE_ROLLBACK_OLD_RENDER";

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "service-k8s.e2e",
    version = "v1",
    kind = "RollbackProbe",
    namespaced,
    status = "RollbackProbeStatus"
)]
pub struct RollbackProbeSpec {
    pub objects: Vec<Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct RollbackProbeStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub rendered_objects: i64,
}

impl ManagedService for RollbackProbe {
    const MANAGER: &'static str = "upgrade-rollback-probe";

    fn render(&self) -> Vec<Value> {
        self.spec.objects.clone()
    }

    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        vec![]
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": { "renderedObjects": self.spec.objects.len() } })
    }
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub body: Value,
}

pub type Log = Arc<Mutex<Vec<Recorded>>>;

pub fn fake_apiserver(
    route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static,
) -> (Client, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let seen = seen.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = route(&method, &path);
            seen.lock().unwrap().push(Recorded { method, path, body });
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, NAMESPACE), log)
}

/// A CR in the harness namespace with no status yet.
pub fn probe(objects: Vec<Value>) -> Arc<RollbackProbe> {
    let mut obj = RollbackProbe::new("probe", RollbackProbeSpec { objects });
    obj.metadata.namespace = Some(NAMESPACE.to_string());
    obj.metadata.uid = Some("uid-upgrade-rollback".to_string());
    obj.metadata.generation = Some(1);
    obj.status = None;
    Arc::new(obj)
}

/// An election that holds the lease, so `reconcile_once` applies the render.
pub fn leader() -> Arc<Election> {
    let election = Election::new("upgrade-rollback-harness".to_string());
    election
        .is_leader
        .store(true, std::sync::atomic::Ordering::Relaxed);
    election
}

/// Reads an old render file: a non-empty JSON array whose items are all objects.
pub fn load_old_render(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("old render {}: {e}", path.display()));
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("old render {}: {e}", path.display()));
    match value {
        Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object) => items,
        _ => panic!("old render {}: not a JSON array of objects", path.display()),
    }
}

/// The old render named by `OLD_RENDER_ENV`, or `None` when it is unset or empty.
pub fn old_render_from_env() -> Option<Vec<Value>> {
    match std::env::var(OLD_RENDER_ENV) {
        Ok(value) if !value.is_empty() => Some(load_old_render(Path::new(&value))),
        _ => None,
    }
}

/// Applies `objects` through the current build's `reconcile_once`.
pub async fn reconcile_with(client: Client, objects: Vec<Value>) -> Result<(), Error> {
    reconcile_once(client, probe(objects), leader())
        .await
        .map(|_| ())
}

/// The objects the current build renders for the probe. `render_dump` prints the
/// same two objects from a worktree of the old tag.
pub fn current_render() -> Vec<Value> {
    let cx = RenderCtx::new(
        "probe",
        "upgrade-rollback-probe",
        "service-k8s.e2e/v1",
        "RollbackProbe",
        "probe",
        NAMESPACE,
    );
    vec![
        service_account(&cx, "serving"),
        client_service(&cx, "probe-client", "serving", 8080),
    ]
}
