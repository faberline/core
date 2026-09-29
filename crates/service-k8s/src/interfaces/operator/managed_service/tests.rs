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
        vec![ReadinessTarget {
            kind: "Deployment",
            name: "pure-render".into(),
        }]
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
    assert_eq!(plan.children, expected);
    assert!(plan.context.is_null());

    let ready = ReadyFacts {
        ready: HashMap::from([("pure-render".into(), 2)]),
    };
    assert_eq!(
        service.status_patch_with_context(&ready, &json!({ "ignored": true })),
        service.status_patch(&ready)
    );
}
