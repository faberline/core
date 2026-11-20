use super::*;

use crate::domain::profile::{
    AlertRule, Capabilities, ExtraWorkload, Partitioning, ProfileError, ReadScaling,
    ServiceProfile, WorkloadKind,
};
use crate::infrastructure::manifest::derived_rbac::derive_rbac_rules;
use crate::infrastructure::manifest::instance_render::{render_instance, InstanceRenderError};
use crate::interfaces::service_spec::ServiceSpec;
use serde_json::{json, Value};

struct Plain;

impl ServiceProfile for Plain {
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

struct Two;

impl ServiceProfile for Two {
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

    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![
            ExtraWorkload {
                name: "collector".to_string(),
                kind: WorkloadKind::Deployment,
            },
            ExtraWorkload {
                name: "sweeper".to_string(),
                kind: WorkloadKind::CronJob,
            },
        ]
    }
}

struct AllKinds;

impl ServiceProfile for AllKinds {
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

    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![
            ExtraWorkload {
                name: "stateful".to_string(),
                kind: WorkloadKind::StatefulSet,
            },
            ExtraWorkload {
                name: "web".to_string(),
                kind: WorkloadKind::Deployment,
            },
            ExtraWorkload {
                name: "agent".to_string(),
                kind: WorkloadKind::DaemonSet,
            },
            ExtraWorkload {
                name: "nightly".to_string(),
                kind: WorkloadKind::CronJob,
            },
        ]
    }
}

struct Bad;

impl ServiceProfile for Bad {
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

    fn extra_workloads() -> Vec<ExtraWorkload> {
        vec![ExtraWorkload {
            name: "config".to_string(),
            kind: WorkloadKind::Deployment,
        }]
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

fn spec() -> ServiceSpec<Value> {
    serde_json::from_value(json!({
        "image": { "image": "lumen:1" },
        "resources": { "cpu": "500m", "memory": "1Gi" },
        "storage": { "size": "10Gi", "class": "fast" },
        "replicas": { "replicasPerShard": 3, "voters": 3 },
        "auth": { "mode": "none" },
        "config": {}
    }))
    .unwrap()
}

fn spec_monitoring() -> ServiceSpec<Value> {
    serde_json::from_value(json!({
        "image": { "image": "lumen:1" },
        "resources": { "cpu": "500m", "memory": "1Gi" },
        "storage": { "size": "10Gi", "class": "fast" },
        "replicas": { "replicasPerShard": 3, "voters": 3 },
        "auth": { "mode": "none" },
        "config": {},
        "monitoring": { "enabled": true }
    }))
    .unwrap()
}

fn kinds(children: &[Value]) -> Vec<&str> {
    children
        .iter()
        .map(|child| child["kind"].as_str().unwrap())
        .collect()
}

fn names(children: &[Value]) -> Vec<&str> {
    children
        .iter()
        .map(|child| child["metadata"]["name"].as_str().unwrap())
        .collect()
}

const CORE_KINDS: [&str; 6] = [
    "ConfigMap",
    "Service",
    "Service",
    "StatefulSet",
    "PodDisruptionBudget",
    "NetworkPolicy",
];

#[test]
fn a_profile_without_extra_workloads_renders_six_children() {
    let children = render_instance::<Plain>(&ctx(), &spec(), 1).unwrap();
    assert_eq!(kinds(&children), CORE_KINDS);
}

#[test]
fn extra_workloads_come_after_the_core_children_in_declaration_order() {
    let children = render_instance::<Two>(&ctx(), &spec(), 1).unwrap();
    assert_eq!(
        kinds(&children),
        [
            "ConfigMap",
            "Service",
            "Service",
            "StatefulSet",
            "PodDisruptionBudget",
            "NetworkPolicy",
            "Deployment",
            "CronJob",
        ]
    );
    assert_eq!(
        names(&children),
        [
            "l1-config",
            "l1-headless",
            "l1",
            "l1",
            "l1",
            "l1",
            "l1-collector",
            "l1-sweeper",
        ]
    );
}

#[test]
fn the_core_children_do_not_change_when_extra_workloads_are_added() {
    let plain = render_instance::<Plain>(&ctx(), &spec(), 1).unwrap();
    let two = render_instance::<Two>(&ctx(), &spec(), 1).unwrap();
    assert_eq!(&two[..6], &plain[..]);
}

#[test]
fn an_extra_workload_pod_has_its_own_component_and_the_instance_settings() {
    let children = render_instance::<Two>(&ctx(), &spec(), 1).unwrap();

    let deployment = &children[6];
    assert_eq!(
        deployment["metadata"]["labels"]["app.kubernetes.io/component"],
        "collector"
    );
    assert_eq!(
        deployment["spec"]["selector"]["matchLabels"]["app.kubernetes.io/component"],
        "collector"
    );
    assert_eq!(deployment["spec"]["replicas"], 1);
    let container = &deployment["spec"]["template"]["spec"]["containers"][0];
    assert_eq!(container["image"], "lumen:1");
    assert_eq!(container["imagePullPolicy"], "IfNotPresent");
    assert_eq!(container["envFrom"][0]["configMapRef"]["name"], "l1-config");
    assert_eq!(container["resources"]["requests"]["cpu"], "500m");
    assert_eq!(container["resources"]["requests"]["memory"], "1Gi");

    let cron = &children[7];
    assert_eq!(cron["spec"]["schedule"], "0 3 * * *");
    assert_eq!(
        cron["metadata"]["labels"]["app.kubernetes.io/component"],
        "sweeper"
    );
}

#[test]
fn all_four_kinds_render_in_order() {
    let children = render_instance::<AllKinds>(&ctx(), &spec(), 1).unwrap();
    assert_eq!(
        kinds(&children[6..]),
        ["StatefulSet", "Deployment", "DaemonSet", "CronJob"]
    );
    assert_eq!(
        names(&children[6..]),
        ["l1-stateful", "l1-web", "l1-agent", "l1-nightly"]
    );

    let stateful = &children[6];
    assert_eq!(stateful["spec"]["serviceName"], "l1-headless");
    assert_eq!(stateful["spec"]["replicas"], 1);
    assert_eq!(
        stateful["spec"]["volumeClaimTemplates"][0]["spec"]["resources"]["requests"]["storage"],
        "10Gi"
    );
}

#[test]
fn derived_rbac_rules_cover_the_extra_kinds() {
    let two = render_instance::<Two>(&ctx(), &spec(), 1).unwrap();
    let rules = derive_rbac_rules(&two);
    let pairs: Vec<(&str, &str)> = rules
        .iter()
        .map(|rule| (rule.api_group.as_str(), rule.resource.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("", "configmaps"),
            ("", "services"),
            ("apps", "deployments"),
            ("apps", "statefulsets"),
            ("batch", "cronjobs"),
            ("networking.k8s.io", "networkpolicies"),
            ("policy", "poddisruptionbudgets"),
        ]
    );

    let plain = render_instance::<Plain>(&ctx(), &spec(), 1).unwrap();
    let plain_rules = derive_rbac_rules(&plain);
    assert!(!plain_rules
        .iter()
        .any(|rule| rule.resource == "deployments" || rule.resource == "cronjobs"));

    let all = render_instance::<AllKinds>(&ctx(), &spec(), 1).unwrap();
    let all_rules = derive_rbac_rules(&all);
    assert!(all_rules
        .iter()
        .any(|rule| rule.api_group == "apps" && rule.resource == "daemonsets"));
}

#[test]
fn a_reserved_extra_workload_name_is_refused_before_rendering() {
    assert_eq!(
        render_instance::<Bad>(&ctx(), &spec(), 1),
        Err(InstanceRenderError::Profile(
            ProfileError::WorkloadNameCollision {
                name: "config".to_string()
            }
        ))
    );
}

#[test]
fn extra_workloads_come_after_the_monitoring_objects() {
    let children = render_instance::<Two>(&ctx(), &spec_monitoring(), 1).unwrap();
    assert_eq!(
        kinds(&children),
        [
            "ConfigMap",
            "Service",
            "Service",
            "StatefulSet",
            "PodDisruptionBudget",
            "NetworkPolicy",
            "ServiceMonitor",
            "PrometheusRule",
            "Deployment",
            "CronJob",
        ]
    );
}
