use super::*;

#[derive(
    kube::CustomResource, Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "CountedService",
    namespaced
)]
struct CountedServiceSpec {
    replicas: u32,
}

impl ManagedService for CountedService {
    const MANAGER: &'static str = "counted-operator";

    fn render(&self) -> Vec<Value> {
        vec![json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "counted" },
            "spec": { "replicas": self.spec.replicas },
        })]
    }

    fn readiness_targets(&self) -> Vec<service::ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": {} })
    }
}

fn counted_ctx(client: Client, leader: bool) -> Arc<Ctx> {
    let election = Election::new("test-identity".to_string());
    election.is_leader.store(leader, Ordering::Relaxed);
    let recorder = Recorder::new(
        client.clone(),
        Reporter {
            controller: CountedService::MANAGER.to_string(),
            instance: Some("test-identity".to_string()),
        },
    );
    Arc::new(Ctx {
        client,
        election,
        metrics: Arc::new(ControllerMetrics::new(CountedService::MANAGER)),
        recorder,
    })
}

fn counted_obj() -> Arc<CountedService> {
    let mut obj = CountedService::new("counted", CountedServiceSpec { replicas: 1 });
    obj.metadata.namespace = Some("acme".to_string());
    Arc::new(obj)
}

/// The error counter is the numerator of the alert that pages a human when
/// a control plane stops converging. Before #2620 `error_policy` discarded
/// the error and returned the same `Action` either way — so asserting on
/// the return value alone would pass against the unfixed code, and the
/// counter is the only observation that actually distinguishes them.
#[tokio::test]
async fn a_failed_reconcile_is_counted_rather_than_discarded() {
    let (client, _) = fake_apiserver(vec![(200, json!({}))]);
    let ctx = counted_ctx(client, true);
    assert_eq!(ctx.metrics.reconcile_errors_total(), 0);

    let action = error_policy::<CountedService>(
        counted_obj(),
        &Error::Missing("metadata.namespace"),
        ctx.clone(),
    );

    assert_eq!(ctx.metrics.reconcile_errors_total(), 1);
    assert_eq!(action, Action::requeue(Duration::from_secs(15)));
}

/// Both operator replicas run the watch loop, but only the leader applies.
/// If a follower's no-op counted as a reconcile, the idle replica would
/// report a steadily climbing `_reconcile_total` with zero errors — which
/// is exactly what a healthy working operator looks like, on the replica
/// that has never touched the cluster.
#[tokio::test]
async fn a_follower_replica_records_no_reconcile_at_all() {
    let (client, seen) = fake_apiserver(vec![]);
    let ctx = counted_ctx(client, false);

    let action = reconcile_entry::<CountedService>(counted_obj(), ctx.clone())
        .await
        .expect("a follower short-circuits successfully");

    assert_eq!(action, Action::requeue(Duration::from_secs(10)));
    assert_eq!(ctx.metrics.reconcile_total(), 0);
    assert!(
        seen.lock().unwrap().is_empty(),
        "a follower must not talk to the apiserver: {:?}",
        seen.lock().unwrap()
    );
}

/// The denominator counts attempts, not successes. A reconcile that fails
/// against the apiserver still took time and still happened, so it has to
/// land in `_reconcile_total` and in the duration histogram — otherwise an
/// operator failing everything divides by zero.
#[tokio::test]
async fn a_leader_counts_the_attempt_even_when_it_fails() {
    let (client, _) = fake_apiserver(vec![(
        500,
        json!({ "kind": "Status", "status": "Failure", "code": 500 }),
    )]);
    let ctx = counted_ctx(client, true);

    let result = reconcile_entry::<CountedService>(counted_obj(), ctx.clone()).await;

    assert!(result.is_err(), "the fake apiserver rejected the apply");
    assert_eq!(ctx.metrics.reconcile_total(), 1);
    assert!(ctx
        .metrics
        .render(true)
        .contains("counted_operator_reconcile_duration_seconds_count 1"));
}
