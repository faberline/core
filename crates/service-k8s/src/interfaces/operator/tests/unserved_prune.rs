use super::*;
use crate::app::operator::reconcile_once;
use crate::interfaces::operator::children::{prune_object, PruneOutcome};

// ---- #3079: an unserved prune API is scoped to the prune ----------------

/// A fake apiserver that answers by *route* instead of by queue position,
/// and records each request's body alongside its method and path.
///
/// [`fake_apiserver`] is the right instrument for driving one function
/// through a known sequence of responses. A whole reconcile is a different
/// shape: how many requests it makes is itself under test, so a queue would
/// turn one extra or missing request into a cascade of mismatched responses
/// rather than into the single assertion that failed.
#[allow(clippy::type_complexity)]
fn recording_apiserver(
    route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static,
) -> (Client, Arc<Mutex<Vec<(String, String, Value)>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<kube::client::Body>| {
        let log = log.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = route(&method, &path);
            log.lock().unwrap().push((method, path, body));
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(kube::client::Body::from(
                        serde_json::to_vec(&response).unwrap(),
                    ))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "acme"), seen)
}

#[derive(
    kube::CustomResource, Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "PruningService",
    namespaced
)]
struct PruningServiceSpec {
    /// The conditions this object stands in for having already persisted.
    /// A real service reads them off its own `.status`; carrying them on
    /// the spec lets a test drive the round *after* a blocked one without
    /// a second apiserver round-trip.
    #[serde(default)]
    prior: Vec<service::Condition>,
}

impl ManagedService for PruningService {
    const MANAGER: &'static str = "pruning-operator";

    fn render(&self) -> Vec<Value> {
        vec![json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "pruned-child" },
            "spec": { "replicas": 1 },
        })]
    }

    fn readiness_targets(&self) -> Vec<service::ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": {} })
    }

    /// Declares no conditions of its own — deliberately. The controller's
    /// own condition has to reach status for a service like this one, which
    /// is the half of the gate a service-authored condition never exercises.
    fn prunes(&self) -> Vec<service::PruneTarget> {
        vec![np_target()]
    }

    fn observed_conditions(&self) -> Vec<service::Condition> {
        self.spec.prior.clone()
    }
}

/// Answers a whole `PruningService` reconcile, with the prune GET's reply
/// left to the caller — that response is the only variable across these
/// cases.
fn pruning_routes(prune: (u16, Value)) -> impl Fn(&str, &str) -> (u16, Value) + Send + Sync {
    move |method: &str, path: &str| match (method, path) {
        ("PATCH", p) if p.ends_with("/deployments/pruned-child") => (
            200,
            json!({ "apiVersion": "apps/v1", "kind": "Deployment",
                    "metadata": { "name": "pruned-child", "namespace": "acme" } }),
        ),
        ("GET", p) if p.ends_with("/networkpolicies/search") => prune.clone(),
        ("PATCH", p) if p.ends_with("/pruningservices/pruned/status") => (
            200,
            json!({ "apiVersion": "service-k8s.test/v1", "kind": "PruningService",
                    "metadata": { "name": "pruned", "namespace": "acme" },
                    "spec": {} }),
        ),
        // Narration, not the subject: answered so it neither fails nor
        // hides from the request log.
        ("POST", p) if p.ends_with("/events") => (201, json!({ "metadata": { "name": "e" } })),
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    }
}

fn pruning_obj(prior: Vec<service::Condition>) -> Arc<PruningService> {
    let mut obj = PruningService::new("pruned", PruningServiceSpec { prior });
    obj.metadata.namespace = Some("acme".to_string());
    // Pruning is gated on the CR's own UID — the controller re-checks the
    // live object's controller `ownerReference` against it.
    obj.metadata.uid = Some("uid-1234".to_string());
    obj.metadata.generation = Some(3);
    Arc::new(obj)
}

/// An election that holds the lease. Leadership is `reconcile_once`'s
/// parameter now, so a unit test that wants a leader's pass says so here
/// rather than relying on the function to elect itself.
fn leader() -> Arc<Election> {
    let election = Election::new("unit-test".to_string());
    election.is_leader.store(true, Ordering::Relaxed);
    election
}

#[allow(clippy::type_complexity)]
fn requests(log: &Arc<Mutex<Vec<(String, String, Value)>>>) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .map(|(m, p, _)| format!("{m} {p}"))
        .collect()
}

/// The status subresource body the reconcile actually sent, or a panic
/// naming every request it did send instead.
#[allow(clippy::type_complexity)]
fn status_write(log: &Arc<Mutex<Vec<(String, String, Value)>>>) -> Value {
    let writes: Vec<Value> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, p, _)| m == "PATCH" && p.ends_with("/pruningservices/pruned/status"))
        .map(|(_, _, body)| body.clone())
        .collect();
    assert_eq!(
        writes.len(),
        1,
        "expected exactly one status write; the reconcile issued {:?}",
        requests(log)
    );
    writes.into_iter().next().unwrap()
}

/// A 404 that reaches `prune_object` is not "the object is absent" —
/// `get_opt` has already turned that shape into `Ok(None)`. It is the
/// apiserver mux declining to route the request, i.e. an API this cluster
/// does not serve, and nothing about that is an operator failure.
#[tokio::test]
async fn an_unserved_prune_api_is_not_a_reconcile_error() {
    let (client, seen) = fake_apiserver(vec![unserved_api()]);
    let outcome = prune_object(&client, "acme", "uid-1234", &np_target()).await;
    assert!(
        outcome.is_ok(),
        "an API the cluster does not serve is a cluster fact, not an \
         operator error: {outcome:?}"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "nothing can be deleted through an API nothing serves"
    );
}

/// The discriminator is the status code, not the reason string. A 404 that
/// *does* parse as a `Status` but carries no `NotFound` reason is the same
/// situation reported by a different layer, and must classify identically.
#[tokio::test]
async fn a_status_shaped_404_without_a_reason_classifies_the_same_way() {
    let (client, seen) = fake_apiserver(vec![reasonless_404()]);
    let outcome = prune_object(&client, "acme", "uid-1234", &np_target()).await;
    assert!(
        outcome.is_ok(),
        "matching kube-client's reconstructed reason string instead of the \
         code would split this apart from the plain-text 404: {outcome:?}"
    );
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// Not an error is not the same as done. If an unserved target came back
/// as `Settled`, the reconcile would have nothing to report and the CR
/// would claim convergence while the enforcement object it wanted gone is
/// still up, unobserved.
///
/// This is the assertion the negative control turns red: it is a panic,
/// not a compile error, because the classification and the return shape
/// are separate.
#[tokio::test]
async fn an_unserved_prune_target_is_reported_rather_than_swallowed() {
    for shape in [unserved_api(), reasonless_404()] {
        let (client, _) = fake_apiserver(vec![shape.clone()]);
        let outcome = prune_object(&client, "acme", "uid-1234", &np_target())
            .await
            .expect("an unserved API is not an operator error");
        assert_eq!(
            outcome,
            PruneOutcome::Unavailable,
            "an outcome the caller cannot distinguish from success is one \
             nobody can report: {shape:?}"
        );
    }

    // …and the endings that really are done still say so, so the variant
    // above is a discrimination rather than a relabelling of every prune.
    let (client, _) = fake_apiserver(vec![not_found()]);
    assert_eq!(
        prune_object(&client, "acme", "uid-1234", &np_target())
            .await
            .expect("absent is success"),
        PruneOutcome::Settled
    );
    let (client, _) = fake_apiserver(vec![(200, live_policy("uid-somebody-else", true))]);
    assert_eq!(
        prune_object(&client, "acme", "uid-1234", &np_target())
            .await
            .expect("a foreign object is a no-op"),
        PruneOutcome::Foreign
    );
}

/// A denied verb is a missing RBAC grant: a human has to fix it, and the
/// reconcile-error counter is how they find out. It stays fatal.
#[tokio::test]
async fn a_denied_prune_stays_fatal() {
    let (client, _) = fake_apiserver(vec![forbidden()]);
    let outcome = prune_object(&client, "acme", "uid-1234", &np_target()).await;
    assert!(
        outcome.is_err(),
        "403 is actionable and must keep failing the reconcile: {outcome:?}"
    );
}

/// A downed aggregated backend should be alerting, not silently absorbed
/// into a status condition.
#[tokio::test]
async fn a_backend_failure_on_a_prune_stays_fatal() {
    let (client, _) = fake_apiserver(vec![service_unavailable()]);
    let outcome = prune_object(&client, "acme", "uid-1234", &np_target()).await;
    assert!(
        outcome.is_err(),
        "503 is actionable and must keep failing the reconcile: {outcome:?}"
    );
}

/// The whole point, at reconcile scope: the pass converges everything it
/// can and reports the one thing it could not, instead of aborting before
/// it has written any status at all.
#[tokio::test]
async fn an_unserved_prune_target_reaches_the_status_as_a_condition() {
    let (client, log) = recording_apiserver(pruning_routes(unserved_api()));

    // The result is checked after the writes, deliberately: the symptom of
    // the regression this pins is a CR left with no status subresource at
    // all, and asserting the result first would report the error instead.
    let action = reconcile_once(client, pruning_obj(Vec::new()), leader()).await;

    let status = status_write(&log);
    let conditions = status["status"]["conditions"]
        .as_array()
        .unwrap_or_else(|| panic!("status carried no conditions array: {status}"))
        .clone();
    assert_eq!(conditions.len(), 1, "{status}");
    assert_eq!(conditions[0]["type"], "PruneBlocked", "{status}");
    assert_eq!(conditions[0]["status"], "True", "{status}");
    assert_eq!(conditions[0]["reason"], "ApiNotServed", "{status}");
    assert_eq!(conditions[0]["observedGeneration"], 3, "{status}");
    let message = conditions[0]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("networking.k8s.io/v1") && message.contains("search"),
        "the condition has to name the target that was not pruned: {status}"
    );

    assert_eq!(
        action.expect("an unserved API is not an operator error"),
        Action::requeue(Duration::from_secs(30)),
        "the prune retries on the next pass, once the API appears"
    );
}

/// The recovery round. `Patch::Merge` replaces an array only when the array
/// is re-sent, so a controller-authored condition that stops applying has
/// to be actively cleared — otherwise the CR keeps reporting `PruneBlocked`
/// forever against an API that came back.
#[tokio::test]
async fn a_recovered_prune_api_clears_the_block_it_left_behind() {
    let blocked = service::Condition {
        type_: "PruneBlocked".into(),
        status: "True".into(),
        reason: "ApiNotServed".into(),
        message: "networking.k8s.io/v1 NetworkPolicy/search".into(),
        last_transition_time: "2026-01-01T00:00:00Z".into(),
        observed_generation: Some(3),
    };
    let (client, log) = recording_apiserver(pruning_routes(not_found()));

    reconcile_once(client, pruning_obj(vec![blocked]), leader())
        .await
        .expect("the API is served again and the object is already gone");

    let status = status_write(&log);
    assert_eq!(
        status["status"]["conditions"],
        json!([]),
        "the recovered round has to re-send the array to empty it: {status}"
    );
}

/// The compatibility control. A service that declares no conditions, with
/// nothing blocked, must write byte-for-byte the status shape it wrote
/// before #3079 — no `conditions` key at all.
#[tokio::test]
async fn nothing_blocked_leaves_the_conditions_array_absent() {
    let (client, log) = recording_apiserver(pruning_routes(not_found()));

    reconcile_once(client, pruning_obj(Vec::new()), leader())
        .await
        .expect("an absent object at a served API is the converged state");

    let status = status_write(&log);
    assert!(
        status["status"].get("conditions").is_none(),
        "a converged pass must not start writing an empty conditions array \
         onto every service in the kit: {status}"
    );
}
