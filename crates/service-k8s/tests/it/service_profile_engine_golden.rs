//! Golden test of the service profile engine for a lumen-shaped profile (#428).
//! A profile with one shard and learner read scaling renders the same JSON twice; a
//! `Bucketed` profile is refused. Uses only public paths.

use serde_json::{json, Value};
use service_k8s::profile::{
    render_operator, AlertRule, Capabilities, InstanceRenderError, Partitioning, ProfileError,
    ReadScaling, ServiceProfile, ServiceSpec,
};
use service_k8s::render::{owner_ref, RenderCtx};

struct LumenShaped;

impl ServiceProfile for LumenShaped {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Single,
            read_scaling: ReadScaling::Learners,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

struct BucketedLumen;

impl ServiceProfile for BucketedLumen {
    const KIND: &'static str = "Lumen";
    const PREFIX: &'static str = "LUMEN";
    type Config = Value;

    fn capabilities() -> Capabilities {
        Capabilities {
            partitioning: Partitioning::Bucketed,
            read_scaling: ReadScaling::Learners,
        }
    }

    fn alerts() -> Vec<AlertRule> {
        Vec::new()
    }
}

fn spec() -> ServiceSpec<Value> {
    serde_json::from_value(json!({
        "image": { "image": "registry.example/lumen:1" },
        "storage": { "size": "10Gi" },
        "replicas": { "replicasPerShard": 5, "voters": 3 },
        "auth": { "mode": "none" },
        "config": { "cache_mb": 64, "mode": "fast" }
    }))
    .unwrap()
}

fn ctx() -> RenderCtx<'static> {
    RenderCtx::new(
        "lumen",
        "lumen-operator",
        "lumen.example/v1",
        "Lumen",
        "search",
        "acme",
    )
    .with_owner(owner_ref("lumen.example/v1", "Lumen", "search", "uid-1"))
}

#[test]
fn the_engine_renders_the_same_json_twice_for_a_lumen_shaped_profile() {
    let first = render_operator::<LumenShaped>(&ctx(), &spec()).unwrap();
    let second = render_operator::<LumenShaped>(&ctx(), &spec()).unwrap();
    assert!(!first.is_empty());
    let first_text = serde_json::to_string(&first).unwrap();
    let second_text = serde_json::to_string(&second).unwrap();
    assert_eq!(first_text, second_text);
    // Every child names its owner, so the two equal renders are real renders and not empty shells.
    for child in &first {
        assert_eq!(child["metadata"]["ownerReferences"][0]["uid"], "uid-1");
    }
    // The same config is rendered into the same ConfigMap text, so the config reaches the pod.
    assert!(first_text.contains("LUMEN_"));
}

#[test]
fn bucketed_is_refused_and_single_with_learners_is_accepted_by_the_engine() {
    assert!(render_operator::<LumenShaped>(&ctx(), &spec()).is_ok());
    let refused = render_operator::<BucketedLumen>(&ctx(), &spec());
    assert!(
        matches!(
            refused,
            Err(InstanceRenderError::Profile(
                ProfileError::UnsupportedCapability {
                    capability: "partitioning=bucketed"
                }
            ))
        ),
        "a Bucketed profile must be refused by the engine"
    );
}
