use super::*;
use serde_json::json;
use std::sync::Mutex;

use serde_json::Value;

use crate::service::{self, ReadyFacts};

mod api_resource;
mod prune;
mod reconcile_metrics;
mod unserved_prune;

/// A fake apiserver that replays `responses` in order and records the
/// method+path of every request, so a test can assert on the call that was
/// *not* made — which is the whole point of an ownership guard.
fn fake_apiserver(responses: Vec<(u16, Value)>) -> (Client, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let queue = Arc::new(Mutex::new(responses.into_iter()));
    let service = tower::service_fn(move |req: http::Request<kube::client::Body>| {
        let log = log.clone();
        let queue = queue.clone();
        async move {
            log.lock()
                .unwrap()
                .push(format!("{} {}", req.method(), req.uri().path()));
            let (code, body) = queue.lock().unwrap().next().unwrap_or((500, json!({})));
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(kube::client::Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "acme"), seen)
}

fn np_target() -> service::PruneTarget {
    service::PruneTarget {
        api_version: "networking.k8s.io/v1",
        kind: "NetworkPolicy",
        name: "search".to_string(),
    }
}

fn live_policy(owner_uid: &str, controller: bool) -> Value {
    json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": "search",
            "namespace": "acme",
            "ownerReferences": [{
                "apiVersion": "lumen.dev/v1alpha1",
                "kind": "Lumen",
                "name": "search",
                "uid": owner_uid,
                "controller": controller,
            }],
        },
    })
}

fn not_found() -> (u16, Value) {
    (
        404,
        json!({ "kind": "Status", "status": "Failure", "message": "not found",
                "reason": "NotFound", "code": 404 }),
    )
}

/// The *other* 404 (#3079). [`not_found`] above is the one a resource
/// handler writes when the API is served and the object is not there —
/// a well-formed `Status` reading `NotFound`, which is the single shape
/// `get_opt` swallows into `Ok(None)`.
///
/// This one is written by the apiserver mux, because nothing routed the
/// request at all: a cluster that does not serve the group answers with its
/// plain `404 page not found` body, which does not parse as a `Status`. Two
/// fixtures rather than one edited fixture, because telling these apart is
/// the entire discrimination the prune path makes.
fn unserved_api() -> (u16, Value) {
    (404, json!("404 page not found"))
}

/// A third 404: `Status`-shaped, but carrying no `reason` — what an
/// aggregation layer or a proxy in front of the apiserver returns. It is
/// still a 404 nothing routed, `get_opt` still hands it back as an error,
/// and it must classify exactly like [`unserved_api`]. Keying on the
/// `"Failed to parse error data"` string kube-client reconstructs for an
/// unparseable body would split these two apart on body syntax alone.
fn reasonless_404() -> (u16, Value) {
    (
        404,
        json!({ "kind": "Status", "status": "Failure",
                "message": "the server could not find the requested resource",
                "code": 404 }),
    )
}

/// A denied verb: a missing RBAC grant, which a human fixes.
fn forbidden() -> (u16, Value) {
    (
        403,
        json!({ "kind": "Status", "status": "Failure",
                "message": "networkpolicies.networking.k8s.io is forbidden",
                "reason": "Forbidden", "code": 403 }),
    )
}

/// A downed backend behind an aggregated API, which should be alerting.
fn service_unavailable() -> (u16, Value) {
    (
        503,
        json!({ "kind": "Status", "status": "Failure",
                "message": "the server is currently unable to handle the request",
                "reason": "ServiceUnavailable", "code": 503 }),
    )
}
