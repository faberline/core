use super::*;
use std::collections::HashMap;

use http::{Request, Response};
use kube::client::Body;
use tower::service_fn;

use crate::application::condition::status_patch::status_patch;
use crate::domain::condition::ConditionStatus;
use crate::domain::profile::{AlertRule, Capabilities, Partitioning, ReadScaling};
use crate::infrastructure::manifest::instance_render::render_operator;

fn inert_client() -> Client {
    let service = service_fn(|_request: Request<Body>| async move {
        Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
    });
    Client::new(service, "default")
}

#[derive(Clone, Debug)]
struct Lumen;

impl ServiceProfile for Lumen {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

impl ProfileApi for Lumen {
    const GROUP: &'static str = "lumen.example";
    const MANAGER: &'static str = "lumen-operator";
    const CRD_NAME: &'static str = "lumens.lumen.example";

    fn domain_dimensions(_spec: &ServiceSpec<Value>, ready_replicas: u32) -> Vec<ConditionFact> {
        let status = if ready_replicas > 0 {
            ConditionStatus::True
        } else {
            ConditionStatus::False
        };
        vec![ConditionFact::new("ShardMapHealthy", status, "Map", "")]
    }
}

/// A profile that declares `partitioning=bucketed`: `validate_profile` refuses it.
#[derive(Clone, Debug)]
struct Bucketed;

impl ServiceProfile for Bucketed {
    const KIND: &'static str = "Shardy";
    const PREFIX: &'static str = "SHARDY";
    type Config = Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Bucketed,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

impl ProfileApi for Bucketed {
    const GROUP: &'static str = "shardy.example";
    const MANAGER: &'static str = "shardy-operator";
    const CRD_NAME: &'static str = "shardies.shardy.example";
}

/// A profile with an empty `KIND`: `validate_profile` refuses it too.
#[derive(Clone, Debug)]
struct NoKind;

impl ServiceProfile for NoKind {
    const KIND: &'static str = "";
    const PREFIX: &'static str = "NOKIND";
    type Config = Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::None,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

impl ProfileApi for NoKind {
    const GROUP: &'static str = "nokind.example";
    const MANAGER: &'static str = "nokind-operator";
    const CRD_NAME: &'static str = "nokinds.nokind.example";
}

fn service<P: ProfileApi<Config = Value>>() -> ProfileService<P> {
    serde_json::from_value(json!({
        "metadata": { "name": "search", "namespace": "acme", "uid": "u1" },
        "spec": {
            "image": { "image": "lumen:1" },
            "storage": { "size": "1Gi" },
            "auth": { "mode": "none" },
            "config": {}
        }
    }))
    .unwrap()
}

#[tokio::test]
async fn reconcile_applies_the_rendered_children() {
    let svc = service::<Lumen>();
    let api_version = "lumen.example/v1";
    let cx = RenderCtx::new(
        "lumen",
        "lumen-operator",
        api_version,
        "Lumen",
        "search",
        "acme",
    )
    .with_owner(owner_ref(api_version, "Lumen", "search", "u1"));
    let expected = render_operator::<Lumen>(&cx, &svc.spec).unwrap();

    let plan = svc.reconcile_plan(inert_client()).await.unwrap();

    assert!(!expected.is_empty());
    assert_eq!(plan.children(), expected.as_slice());
    assert_eq!(svc.render(), expected);
}

#[test]
fn reconcile_sets_the_standard_conditions_in_status() {
    let svc = service::<Lumen>();
    let name = svc.readiness_targets()[0].name().to_string();
    let ready = ReadyFacts::new(HashMap::from([(name, 1)]));

    let patch = status_patch(&svc, &ready, &Value::Null, &[]).unwrap();
    let body = patch.body();

    let conditions = body["status"]["conditions"].as_array().unwrap();
    let types: Vec<&str> = conditions
        .iter()
        .map(|c| c["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "Ready",
            "Progressing",
            "StorageHealthy",
            "BackupConfigured",
            "BackupSucceeded",
            "ShardMapHealthy",
        ]
    );
    let shard_map = conditions.last().unwrap();
    assert_eq!(shard_map["status"], "True");
    assert_eq!(body["status"]["readyReplicas"], 1);
}

#[tokio::test]
async fn a_profile_that_fails_validation_is_not_applied() {
    let svc = service::<Bucketed>();

    assert!(svc.reconcile_plan(inert_client()).await.is_err());
    assert!(svc.render().is_empty());
    assert!(svc.readiness_targets().is_empty());
}

#[tokio::test]
async fn an_empty_kind_is_not_applied() {
    let svc = service::<NoKind>();

    assert!(svc.reconcile_plan(inert_client()).await.is_err());
    assert!(svc.render().is_empty());
}

#[test]
fn the_readiness_target_is_the_statefulset_of_the_render() {
    let svc = service::<Lumen>();
    let targets = svc.readiness_targets();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].kind(), "StatefulSet");
    let rendered = svc
        .render()
        .into_iter()
        .find(|child| child["kind"] == "StatefulSet")
        .unwrap();
    assert_eq!(
        targets[0].name(),
        rendered["metadata"]["name"].as_str().unwrap()
    );
}

#[test]
fn the_resource_identity_comes_from_the_profile() {
    assert_eq!(ProfileService::<Lumen>::crd_name(), "lumens.lumen.example");

    let ar = ProfileService::<Lumen>::api_resource();
    assert_eq!(ar.kind, "Lumen");
    assert_eq!(ar.group, "lumen.example");
    assert_eq!(ar.plural, "lumens");
    assert_eq!(
        <ProfileService<Lumen> as ManagedService>::MANAGER,
        "lumen-operator"
    );
}

#[test]
fn observed_conditions_are_read_back_from_the_status() {
    let mut svc = service::<Lumen>();
    assert!(svc.observed_conditions().is_empty());

    svc.status = Some(json!({
        "conditions": [{
            "type": "Ready",
            "status": "True",
            "reason": "R",
            "message": "",
            "lastTransitionTime": "2026-01-01T00:00:00Z",
            "observedGeneration": 1
        }]
    }));
    let observed = svc.observed_conditions();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].type_, "Ready");
}
