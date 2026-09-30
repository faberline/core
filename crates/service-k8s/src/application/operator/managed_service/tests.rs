use super::*;
use http::{Request, Response};
use kube::client::Body;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower::service_fn;

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "PureRenderService",
    namespaced
)]
struct PureRenderServiceSpec {
    replicas: u32,
}

impl ManagedService for PureRenderService {
    const MANAGER: &'static str = "pure-render-test";

    fn render(&self) -> Vec<serde_json::Value> {
        vec![json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "pure-render" },
            "spec": { "replicas": self.spec.replicas },
        })]
    }

    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        vec![ReadinessTarget::new("Deployment", "pure-render")]
    }

    fn status_patch(&self, ready: &ReadyFacts) -> serde_json::Value {
        json!({ "status": { "readyReplicas": ready.get("pure-render") } })
    }
}

fn inert_client() -> Client {
    let service = service_fn(|_request: Request<Body>| async move {
        Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
    });
    Client::new(service, "default")
}

#[tokio::test]
async fn default_plan_and_status_preserve_existing_contract() {
    let service = PureRenderService::new("pure-render", PureRenderServiceSpec { replicas: 2 });
    let expected = service.render();
    let plan = service
        .reconcile_plan(inert_client())
        .await
        .expect("pure render plan");
    assert_eq!(plan.children(), expected);
    assert!(plan.context().is_null());

    let ready = ReadyFacts::new(HashMap::from([("pure-render".into(), 2)]));
    assert_eq!(
        service.status_patch_with_context(&ready, &json!({ "ignored": true })),
        service.status_patch(&ready)
    );
}

#[test]
fn constructors_keep_what_they_were_given() {
    let target = ReadinessTarget::new("StatefulSet", "db-store");
    assert_eq!((target.kind(), target.name()), ("StatefulSet", "db-store"));

    let prune = PruneTarget::new("networking.k8s.io/v1", "NetworkPolicy", "db");
    assert_eq!(
        (prune.api_version(), prune.kind(), prune.name()),
        ("networking.k8s.io/v1", "NetworkPolicy", "db")
    );

    let child = ClusterScopedChild::new(
        "rbac.authorization.k8s.io/v1",
        "ClusterRoleBinding",
        "db.auth-delegator",
        false,
    );
    assert!(child.expected_labels().is_empty());
    assert!(!child.desired());
    let child = child
        .with_expected_labels(BTreeMap::from([("a".to_string(), "1".to_string())]))
        .with_expected_labels(BTreeMap::from([("b".to_string(), "2".to_string())]));
    assert_eq!(
        child.expected_labels().keys().collect::<Vec<_>>(),
        ["a", "b"]
    );

    let plan = ReconcilePlan::new(vec![json!({"kind": "Service"})], json!({"k": 1}));
    let (children, context) = plan.into_parts();
    assert_eq!(children, vec![json!({"kind": "Service"})]);
    assert_eq!(context, json!({"k": 1}));

    let ready = ReadyFacts::new(HashMap::from([("db-store".to_string(), 3)]));
    assert_eq!(ready.get("db-store"), 3);
    assert_eq!(ready.get("absent"), 0);
    assert_eq!(ready.ready().len(), 1);
}
