//! The Fleet example Role is namespaced and grants no cluster-wide write (#645).
//!
//! `examples/fleet-role.yaml` is the RBAC a Fleet operator needs for one target
//! namespace. These tests read the example, the README, the roadmap and the
//! status table as text. They need no cluster and no network.

use serde::Deserialize;

const EXAMPLE: &str = include_str!("../../examples/fleet-role.yaml");
const README: &str = include_str!("../../README.md");
const ROADMAP: &str = include_str!("../../ROADMAP.md");
const STATUS: &str = include_str!("../../STATUS.md");

fn documents() -> Vec<serde_yaml::Value> {
    serde_yaml::Deserializer::from_str(EXAMPLE)
        .map(|doc| serde_yaml::Value::deserialize(doc).expect("each document parses"))
        .collect()
}

fn str_at<'a>(value: &'a serde_yaml::Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

#[test]
fn fleet_role_example_has_only_role_and_rolebinding_kinds() {
    let docs = documents();
    let kinds: Vec<&str> = docs
        .iter()
        .map(|doc| str_at(doc, &["kind"]).expect("every document has a kind"))
        .collect();
    assert_eq!(kinds, ["Role", "RoleBinding", "Role", "RoleBinding"]);

    assert!(
        !EXAMPLE.contains("ClusterRole"),
        "the example must not name a ClusterRole or a ClusterRoleBinding"
    );

    for doc in &docs {
        let Some(rules) = doc.get("rules").and_then(|rules| rules.as_sequence()) else {
            continue;
        };
        for rule in rules {
            for field in ["verbs", "resources", "apiGroups"] {
                let values = rule
                    .get(field)
                    .and_then(|values| values.as_sequence())
                    .into_iter()
                    .flatten();
                for value in values {
                    assert_ne!(value.as_str(), Some("*"), "wildcard in {field}");
                }
            }
        }
    }
}

#[test]
fn fleet_role_example_namespaces_every_object() {
    for doc in documents() {
        let namespace = str_at(&doc, &["metadata", "namespace"]).unwrap_or("");
        assert!(!namespace.is_empty(), "every object names its namespace");

        if str_at(&doc, &["kind"]) == Some("RoleBinding") {
            let subjects = doc
                .get("subjects")
                .and_then(|subjects| subjects.as_sequence())
                .expect("a RoleBinding has subjects");
            for subject in subjects {
                assert_eq!(str_at(subject, &["kind"]), Some("ServiceAccount"));
            }
        }
    }
}

#[test]
fn readme_names_the_fleet_module_and_the_role_example() {
    assert!(README.contains("run_fleet"), "README names run_fleet");
    assert!(
        README.contains("KubeFleetCluster"),
        "README names KubeFleetCluster"
    );
    assert!(
        README.contains("examples/fleet-role.yaml"),
        "README links the example"
    );
    assert!(
        !README.contains("There is no `service_k8s::fleet` module today"),
        "the README no longer says the Fleet module is missing"
    );
}

#[test]
fn status_keeps_shared_fleet_controller_not_supported() {
    assert!(
        STATUS
            .lines()
            .any(|line| line.contains("shared-fleet-controller") && line.contains("Not supported")),
        "STATUS.md still lists shared-fleet-controller as Not supported"
    );
}

#[test]
fn roadmap_tracks_the_fleet_outcome_in_639() {
    let start = ROADMAP
        .find("### Shared Fleet controller")
        .expect("ROADMAP has the Shared Fleet controller section");
    let section = ROADMAP[start..]
        .split("\n### ")
        .next()
        .expect("the section has a body");
    assert!(
        section.contains("https://github.com/faberline/core/issues/639"),
        "the Shared Fleet controller section tracks #639"
    );
}
