//! W0 golden bytes for the status subresource patch one reconcile pass writes.
//!
//! The service's `status_patch` and the projected `status.conditions[]` are
//! merged into one `Patch::Merge` body. The case pins that body as the
//! apiserver receives it, byte for byte, for fixed readiness and fixed
//! conditions. Every declared fact keeps the status it was persisted with, so
//! `project` carries each transition time forward and the body does not depend
//! on the clock.
//!
//! `status_conditions_survive_the_recovery_pass.rs` pins which conditions a
//! pass keeps; this file pins the whole body those conditions travel in.

#![cfg(feature = "controller")]

use std::sync::{Arc, Mutex};

use kube::client::Body;
use kube::{Client, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use service_k8s::controller::reconcile_once;
use service_k8s::service::{
    Condition, ConditionFact, ConditionStatus, ManagedService, ReadinessTarget, ReadyFacts,
};
use service_k8s::Election;

const NAMESPACE: &str = "acme";

/// Every request the fake apiserver saw: method, path and the raw body text.
type Log = Arc<Mutex<Vec<(String, String, String)>>>;

fn fake_apiserver() -> (Client, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let seen = seen.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = String::from_utf8(bytes.to_vec()).unwrap_or_default();
            let (code, response) = if method == "POST" && path.ends_with("/events") {
                (201, json!({ "metadata": { "name": "e" } }))
            } else if method == "PATCH" && path.ends_with("/goldenservices/golden/status") {
                (
                    200,
                    json!({ "apiVersion": "service-k8s.e2e/v1", "kind": "GoldenService",
                              "metadata": { "name": "golden", "namespace": NAMESPACE },
                              "spec": { "tier": "gold" } }),
                )
            } else {
                (
                    500,
                    json!({ "kind": "Status", "status": "Failure", "code": 500 }),
                )
            };
            seen.lock().unwrap().push((method, path, body));
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

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "service-k8s.e2e",
    version = "v1",
    kind = "GoldenService",
    namespaced,
    status = "GoldenServiceStatus"
)]
struct GoldenServiceSpec {
    tier: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
struct GoldenServiceStatus {
    #[serde(default)]
    conditions: Vec<Condition>,
}

impl ManagedService for GoldenService {
    const MANAGER: &'static str = "golden-e2e-operator";

    /// Nothing to apply: the case is about the status body, not the children.
    fn render(&self) -> Vec<Value> {
        Vec::new()
    }

    /// No workloads, so the ready facts are fixed: every lookup reads 0.
    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, ready: &ReadyFacts) -> Value {
        json!({ "status": { "readyReplicas": ready.get("golden-child"), "phase": "Running", "tier": self.spec.tier } })
    }

    fn conditions(&self, _ready: &ReadyFacts, _context: &Value) -> Vec<ConditionFact> {
        vec![
            ConditionFact::new(
                "Ready",
                ConditionStatus::True,
                "Serving",
                "every shard is serving",
            ),
            ConditionFact::new("Degraded", ConditionStatus::False, "Healthy", ""),
        ]
    }

    fn observed_conditions(&self) -> Vec<Condition> {
        self.status
            .as_ref()
            .map(|s| s.conditions.clone())
            .unwrap_or_default()
    }
}

fn persisted(type_: &str, status: &str, since: &str) -> Condition {
    Condition {
        type_: type_.to_string(),
        status: status.to_string(),
        reason: "Recorded".to_string(),
        message: "as a previous pass left it".to_string(),
        last_transition_time: since.to_string(),
        observed_generation: Some(2),
    }
}

fn subject() -> Arc<GoldenService> {
    let mut obj = GoldenService::new(
        "golden",
        GoldenServiceSpec {
            tier: "gold".to_string(),
        },
    );
    obj.metadata.namespace = Some(NAMESPACE.to_string());
    obj.metadata.uid = Some("uid-golden".to_string());
    obj.metadata.generation = Some(3);
    obj.status = Some(GoldenServiceStatus {
        conditions: vec![
            persisted("Ready", "True", "2026-01-01T00:00:00Z"),
            persisted("Degraded", "False", "2026-01-02T00:00:00Z"),
        ],
    });
    Arc::new(obj)
}

fn leader() -> Arc<Election> {
    let election = Election::new("status-patch-golden".to_string());
    election
        .is_leader
        .store(true, std::sync::atomic::Ordering::Relaxed);
    election
}

/// The status body, as literal text.
const STATUS_PATCH: &str = concat!(
    r#"{"status":{"readyReplicas":0,"phase":"Running","tier":"gold","conditions":["#,
    r#"{"type":"Ready","status":"True","reason":"Serving","message":"every shard is serving","#,
    r#""lastTransitionTime":"2026-01-01T00:00:00Z","observedGeneration":3},"#,
    r#"{"type":"Degraded","status":"False","reason":"Healthy","message":"","#,
    r#""lastTransitionTime":"2026-01-02T00:00:00Z","observedGeneration":3}]}}"#,
);

#[tokio::test]
async fn the_status_patch_body_is_pinned_byte_for_byte() {
    let (client, log) = fake_apiserver();

    reconcile_once(client, subject(), leader())
        .await
        .expect("a pass with nothing to apply converges");

    let writes: Vec<String> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(method, path, _)| {
            method == "PATCH" && path.ends_with("/goldenservices/golden/status")
        })
        .map(|(_, _, body)| body.clone())
        .collect();
    assert_eq!(writes.len(), 1, "expected exactly one status write");
    assert_eq!(
        writes[0], STATUS_PATCH,
        "the status body the apiserver received"
    );
    let decoded: Value = serde_json::from_str(STATUS_PATCH).unwrap();
    assert_eq!(
        decoded["status"]["conditions"],
        serde_json::to_value(vec![
            Condition {
                type_: "Ready".to_string(),
                status: "True".to_string(),
                reason: "Serving".to_string(),
                message: "every shard is serving".to_string(),
                last_transition_time: "2026-01-01T00:00:00Z".to_string(),
                observed_generation: Some(3),
            },
            Condition {
                type_: "Degraded".to_string(),
                status: "False".to_string(),
                reason: "Healthy".to_string(),
                message: String::new(),
                last_transition_time: "2026-01-02T00:00:00Z".to_string(),
                observed_generation: Some(3),
            },
        ])
        .unwrap(),
        "the literal decodes to the projected conditions"
    );
}
