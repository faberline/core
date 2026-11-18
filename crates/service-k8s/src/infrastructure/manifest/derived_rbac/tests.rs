use super::*;

use serde_json::json;

fn child(api_version: &str, kind: &str) -> Value {
    json!({ "apiVersion": api_version, "kind": kind, "metadata": { "name": "x" } })
}

fn pairs(rules: &[DerivedRule]) -> Vec<(&str, &str)> {
    rules
        .iter()
        .map(|rule| (rule.api_group.as_str(), rule.resource.as_str()))
        .collect()
}

#[test]
fn adding_a_child_kind_adds_its_rule() {
    let two = vec![child("v1", "ConfigMap"), child("apps/v1", "StatefulSet")];
    let rules = derive_rbac_rules(&two);
    assert_eq!(rules.len(), 2);
    let mut three = two.clone();
    three.push(child("policy/v1", "PodDisruptionBudget"));
    let rules = derive_rbac_rules(&three);
    assert_eq!(rules.len(), 3);
    let pdb = rules
        .iter()
        .find(|rule| rule.resource == "poddisruptionbudgets")
        .expect("a rule for the PodDisruptionBudget");
    assert_eq!(pdb.api_group, "policy");
}

#[test]
fn no_rule_exists_for_a_kind_that_is_not_rendered() {
    let with = vec![child("v1", "ConfigMap"), child("apps/v1", "StatefulSet")];
    assert!(derive_rbac_rules(&with)
        .iter()
        .any(|rule| rule.resource == "statefulsets"));
    let without = vec![child("v1", "ConfigMap")];
    let rules = derive_rbac_rules(&without);
    assert!(!rules.iter().any(|rule| rule.resource == "statefulsets"));
    assert_eq!(pairs(&rules), [("", "configmaps")]);
}

#[test]
fn rules_are_sorted_and_distinct() {
    let children = vec![
        child("v1", "Service"),
        child("monitoring.coreos.com/v1", "ServiceMonitor"),
        child("apps/v1", "StatefulSet"),
        child("v1", "Service"),
    ];
    let rules = derive_rbac_rules(&children);
    assert_eq!(
        pairs(&rules),
        [
            ("", "services"),
            ("apps", "statefulsets"),
            ("monitoring.coreos.com", "servicemonitors"),
        ]
    );
    assert_eq!(
        rules
            .iter()
            .filter(|rule| rule.resource == "services")
            .count(),
        1
    );
}

#[test]
fn every_rule_has_the_six_verbs() {
    let rules = derive_rbac_rules(&[child("v1", "ConfigMap")]);
    assert_eq!(
        rules[0].verbs,
        ["get", "list", "watch", "create", "patch", "delete"]
    );
}

#[test]
fn the_kind_table_maps_every_rendered_kind() {
    let children = vec![
        child("v1", "ConfigMap"),
        child("v1", "Service"),
        child("v1", "ServiceAccount"),
        child("apps/v1", "StatefulSet"),
        child("apps/v1", "Deployment"),
        child("policy/v1", "PodDisruptionBudget"),
        child("networking.k8s.io/v1", "NetworkPolicy"),
        child("monitoring.coreos.com/v1", "ServiceMonitor"),
        child("monitoring.coreos.com/v1", "PrometheusRule"),
    ];
    assert_eq!(
        pairs(&derive_rbac_rules(&children)),
        [
            ("", "configmaps"),
            ("", "serviceaccounts"),
            ("", "services"),
            ("apps", "deployments"),
            ("apps", "statefulsets"),
            ("monitoring.coreos.com", "prometheusrules"),
            ("monitoring.coreos.com", "servicemonitors"),
            ("networking.k8s.io", "networkpolicies"),
            ("policy", "poddisruptionbudgets"),
        ]
    );
}

#[test]
fn an_unknown_kind_gets_the_lowercase_kind_with_an_s() {
    let rules = derive_rbac_rules(&[child("example.com/v1", "Widget")]);
    assert_eq!(pairs(&rules), [("example.com", "widgets")]);
}

#[test]
fn a_child_without_api_version_or_kind_is_skipped() {
    let children = vec![
        json!({ "kind": "ConfigMap" }),
        json!({ "apiVersion": "v1" }),
        json!({ "apiVersion": 1, "kind": "ConfigMap" }),
        json!("not an object"),
    ];
    assert!(derive_rbac_rules(&children).is_empty());
}

#[test]
fn no_children_give_no_rules() {
    assert!(derive_rbac_rules(&[]).is_empty());
}
