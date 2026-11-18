use super::*;

use crate::domain::profile::{AlertRule, Capabilities, Partitioning, ReadScaling};

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

fn ctx() -> RenderCtx<'static> {
    RenderCtx::new(
        "lumen",
        "core-operator",
        "lumen.example/v1",
        "Lumen",
        "l1",
        "ns",
    )
}

fn spec_with(replicas_per_shard: u32, config: Value) -> ServiceSpec<Value> {
    serde_json::from_value(json!({
        "image": { "image": "lumen:1" },
        "resources": { "cpu": "500m", "memory": "1Gi" },
        "storage": { "size": "10Gi", "class": "fast" },
        "replicas": { "replicasPerShard": replicas_per_shard, "voters": 3 },
        "auth": { "mode": "none" },
        "config": config,
    }))
    .unwrap()
}

fn spec() -> ServiceSpec<Value> {
    spec_with(
        3,
        json!({
            "cacheMb": 64,
            "mode": "fast",
            "db": { "password": { "secretName": "lumen-db", "key": "pw" } },
        }),
    )
}

fn kinds(children: &[Value]) -> Vec<&str> {
    children
        .iter()
        .map(|child| child["kind"].as_str().unwrap())
        .collect()
}

#[test]
fn a_spec_with_more_than_one_shard_is_refused() {
    let err = render_instance::<Lumen>(&ctx(), &spec(), 2).unwrap_err();
    assert_eq!(
        err,
        InstanceRenderError::MultiShardUnsupported { shards: 2 }
    );
    assert_eq!(
        err.to_string(),
        "the first release supports one shard, got 2"
    );
}

#[test]
fn zero_shards_is_refused_too() {
    let err = render_instance::<Lumen>(&ctx(), &spec(), 0).unwrap_err();
    assert_eq!(
        err,
        InstanceRenderError::MultiShardUnsupported { shards: 0 }
    );
}

#[test]
fn an_invalid_profile_is_refused_before_anything_else() {
    // shards is also wrong here: the profile error must win.
    let err = render_instance::<NoKind>(&ctx(), &spec(), 2).unwrap_err();
    assert_eq!(err, InstanceRenderError::Profile(ProfileError::EmptyKind));
}

#[test]
fn a_config_that_is_not_an_object_is_refused() {
    let err = render_instance::<Lumen>(&ctx(), &spec_with(3, json!(5)), 1).unwrap_err();
    assert_eq!(
        err,
        InstanceRenderError::Config(ConfigEnvError::NotAnObject)
    );
}

#[test]
fn zero_replicas_per_shard_is_refused_by_the_workload_renderer() {
    let err = render_instance::<Lumen>(&ctx(), &spec_with(0, json!({})), 1).unwrap_err();
    assert_eq!(
        err,
        InstanceRenderError::Workload(StatefulInstanceError::ZeroReplicas)
    );
}

#[test]
fn one_shard_renders_the_six_kinds_in_order() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    assert_eq!(
        kinds(&children),
        [
            "ConfigMap",
            "Service",
            "Service",
            "StatefulSet",
            "PodDisruptionBudget",
            "NetworkPolicy"
        ]
    );
    let names: Vec<&str> = children
        .iter()
        .map(|child| child["metadata"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["l1-config", "l1-headless", "l1", "l1", "l1", "l1"]);
    assert_eq!(children[1]["spec"]["clusterIP"], "None");
    assert_eq!(children[2]["spec"]["type"], "ClusterIP");
    for child in &children {
        assert_eq!(child["metadata"]["namespace"], "ns", "{}", child["kind"]);
        assert_eq!(
            child["metadata"]["labels"]["app.kubernetes.io/component"],
            "server"
        );
    }
}

#[test]
fn config_keys_reach_the_pod_through_env_from() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    let container = &children[3]["spec"]["template"]["spec"]["containers"][0];
    assert_eq!(
        container["envFrom"],
        json!([{ "configMapRef": { "name": "l1-config" } }])
    );
    let data = children[0]["data"].as_object().unwrap();
    assert_eq!(data["LUMEN_CACHE_MB"], "64");
    assert_eq!(data["LUMEN_MODE"], "fast");
    assert_eq!(
        data["LUMEN_DB_PASSWORD"],
        "/etc/core/secrets/lumen-db-password/pw"
    );
    assert_eq!(data.len(), 3);
    assert!(data.keys().all(|key| key.starts_with("LUMEN_")));
}

#[test]
fn a_secret_ref_becomes_a_volume_and_a_mount_in_the_statefulset() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    let pod = &children[3]["spec"]["template"]["spec"];
    let volumes = pod["volumes"].as_array().unwrap();
    assert!(volumes.contains(&json!({
        "name": "secret-lumen-db-password",
        "secret": {
            "secretName": "lumen-db",
            "defaultMode": 0o444,
            "items": [{ "key": "pw", "path": "pw" }],
        },
    })));
    let mounts = pod["containers"][0]["volumeMounts"].as_array().unwrap();
    assert!(mounts.contains(&json!({
        "name": "secret-lumen-db-password",
        "mountPath": "/etc/core/secrets/lumen-db-password",
        "readOnly": true,
    })));
    // The Secret name is never written into the ConfigMap.
    assert!(!children[0]["data"].to_string().contains("lumen-db\""));
}

#[test]
fn replicas_come_from_the_spec() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    let sts = &children[3];
    assert_eq!(sts["spec"]["replicas"], 3);
    assert_eq!(sts["spec"]["serviceName"], "l1-headless");
    let env = sts["spec"]["template"]["spec"]["containers"][0]["env"]
        .as_array()
        .unwrap();
    let value = |name: &str| {
        env.iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("env {name}"))["value"]
            .clone()
    };
    assert_eq!(value("SHARD_COUNT"), "1");
    assert_eq!(value("REPLICAS_PER_SHARD"), "3");
    assert_eq!(value("VOTER_COUNT"), "3");
    assert_eq!(value("LUMEN_HEADLESS_SERVICE"), "l1-headless");
}

#[test]
fn image_resources_and_storage_come_from_the_spec() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    let sts = &children[3];
    let container = &sts["spec"]["template"]["spec"]["containers"][0];
    assert_eq!(container["image"], "lumen:1");
    assert_eq!(container["imagePullPolicy"], "IfNotPresent");
    assert_eq!(container["resources"]["requests"]["cpu"], "500m");
    assert_eq!(container["resources"]["requests"]["memory"], "1Gi");
    assert_eq!(container["ports"][0]["name"], "http");
    assert_eq!(container["ports"][0]["containerPort"], 8080);
    let claim = &sts["spec"]["volumeClaimTemplates"][0];
    assert_eq!(claim["metadata"]["name"], "data");
    assert_eq!(claim["spec"]["resources"]["requests"]["storage"], "10Gi");
    assert_eq!(claim["spec"]["storageClassName"], "fast");
    assert_eq!(claim["spec"]["accessModes"], json!(["ReadWriteOnce"]));
    assert!(container["volumeMounts"]
        .as_array()
        .unwrap()
        .contains(&json!({ "name": "data", "mountPath": "/data", "readOnly": false })));
}

#[test]
fn the_pdb_and_the_network_policy_select_the_instance_pods() {
    let children = render_instance::<Lumen>(&ctx(), &spec(), 1).unwrap();
    let selector = ctx().selector("server");
    assert_eq!(children[4]["spec"]["maxUnavailable"], 1);
    assert_eq!(children[4]["spec"]["selector"]["matchLabels"], selector);
    assert_eq!(children[5]["spec"]["podSelector"]["matchLabels"], selector);
    assert_eq!(children[3]["spec"]["selector"]["matchLabels"], selector);
}
