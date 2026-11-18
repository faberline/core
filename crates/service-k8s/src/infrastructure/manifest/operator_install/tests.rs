use super::*;

use serde_json::{json, Value};

use crate::domain::profile::{AlertRule, Capabilities, Partitioning, ReadScaling};
use crate::infrastructure::manifest::derived_rbac::derive_rbac_rules;
use crate::infrastructure::manifest::instance_render::render_operator;
use crate::infrastructure::manifest::RenderCtx;

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

fn opts() -> InstallOptions {
    InstallOptions {
        namespace: "ops".into(),
        image: "registry.example/lumen-operator:1".into(),
        group: "lumen.example".into(),
    }
}

fn sample(monitoring: bool) -> ServiceSpec<Value> {
    serde_json::from_value(json!({
        "image": { "image": "lumen:1" },
        "storage": { "size": "1Gi" },
        "auth": { "mode": "none" },
        "monitoring": { "enabled": monitoring },
        "config": {},
    }))
    .unwrap()
}

fn kinds(objects: &[Value]) -> Vec<&str> {
    objects
        .iter()
        .map(|o| o["kind"].as_str().unwrap())
        .collect()
}

fn rules(objects: &[Value]) -> &Vec<Value> {
    objects[1]["rules"].as_array().unwrap()
}

fn has_rule(objects: &[Value], group: &str, resource: &str) -> bool {
    rules(objects)
        .iter()
        .any(|r| r["apiGroups"][0] == group && r["resources"][0] == resource)
}

#[test]
fn the_install_manifests_contain_the_deployment_the_crd_and_the_rbac_objects() {
    let objects = operator_install::<Lumen>(&opts(), &sample(false)).unwrap();

    assert_eq!(
        kinds(&objects),
        [
            "ServiceAccount",
            "ClusterRole",
            "ClusterRoleBinding",
            "CustomResourceDefinition",
            "Deployment",
        ]
    );

    let crd = &objects[3];
    assert_eq!(crd["spec"]["group"], "lumen.example");
    assert_eq!(crd["spec"]["names"]["kind"], "Lumen");

    let deployment = &objects[4];
    assert_eq!(deployment["metadata"]["namespace"], "ops");
    assert_eq!(deployment["spec"]["replicas"], 1);
    assert_eq!(
        deployment["spec"]["template"]["spec"]["containers"][0]["image"],
        "registry.example/lumen-operator:1"
    );
    assert_eq!(
        deployment["spec"]["template"]["spec"]["serviceAccountName"],
        "lumen-operator"
    );
    assert_eq!(objects[0]["metadata"]["name"], "lumen-operator");

    let binding = &objects[2];
    assert_eq!(binding["roleRef"]["name"], "lumen-operator");
    assert_eq!(binding["subjects"][0]["namespace"], "ops");

    for cluster_scoped in [&objects[1], &objects[2]] {
        assert!(cluster_scoped["metadata"].get("ownerReferences").is_none());
        assert!(cluster_scoped["metadata"].get("namespace").is_none());
    }
}

#[test]
fn the_cluster_role_covers_every_rendered_child_kind_and_the_profile_kind() {
    let objects = operator_install::<Lumen>(&opts(), &sample(false)).unwrap();
    let child_cx = RenderCtx::new(
        "lumen",
        "lumen-operator",
        "lumen.example/v1",
        "Lumen",
        "sample",
        "ops",
    );
    let children = render_operator::<Lumen>(&child_cx, &sample(false)).unwrap();

    for derived in derive_rbac_rules(&children) {
        assert!(
            has_rule(&objects, &derived.api_group, &derived.resource),
            "missing rule for {}/{}",
            derived.api_group,
            derived.resource
        );
    }
    assert!(has_rule(&objects, "apps", "statefulsets"));
    assert!(has_rule(&objects, "lumen.example", "lumens"));
    assert!(has_rule(&objects, "lumen.example", "lumens/status"));
    assert!(has_rule(&objects, "coordination.k8s.io", "leases"));
    assert!(has_rule(&objects, "", "events"));
}

#[test]
fn the_cluster_role_has_a_monitoring_rule_only_when_the_render_has_a_monitoring_child() {
    let without = operator_install::<Lumen>(&opts(), &sample(false)).unwrap();
    assert!(!has_rule(
        &without,
        "monitoring.coreos.com",
        "servicemonitors"
    ));
    assert!(!has_rule(
        &without,
        "monitoring.coreos.com",
        "prometheusrules"
    ));

    let with = operator_install::<Lumen>(&opts(), &sample(true)).unwrap();
    assert!(has_rule(&with, "monitoring.coreos.com", "servicemonitors"));
    assert!(has_rule(&with, "monitoring.coreos.com", "prometheusrules"));
}

#[test]
fn the_install_is_refused_for_an_invalid_profile() {
    let result = operator_install::<NoKind>(&opts(), &sample(false));
    assert!(matches!(
        result,
        Err(InstallError::Profile(ProfileError::EmptyKind))
    ));
}
