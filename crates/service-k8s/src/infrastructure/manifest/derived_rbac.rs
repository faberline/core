//! Derive the operator's RBAC rules from the kinds of the rendered children (ADR 0002 D14).
//!
//! Pure: no I/O. The rules come only from the children given. There is no hand-written
//! list of rendered kinds, so a kind that is not rendered gets no rule.

use std::collections::BTreeSet;

use serde_json::Value;

/// The verbs the operator needs on every child kind it applies.
const DERIVED_VERBS: [&str; 6] = ["get", "list", "watch", "create", "patch", "delete"];

/// Kind to plural resource name. A kind that is not in this table gets
/// `lowercase(kind) + "s"` (see `resource_for_kind`).
const KIND_RESOURCES: [(&str, &str); 9] = [
    ("ConfigMap", "configmaps"),
    ("Service", "services"),
    ("ServiceAccount", "serviceaccounts"),
    ("StatefulSet", "statefulsets"),
    ("Deployment", "deployments"),
    ("PodDisruptionBudget", "poddisruptionbudgets"),
    ("NetworkPolicy", "networkpolicies"),
    ("ServiceMonitor", "servicemonitors"),
    ("PrometheusRule", "prometheusrules"),
];

/// One RBAC rule of the operator: the verbs on one resource of one API group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedRule {
    /// API group. Empty for the core group (`apiVersion: v1`).
    pub api_group: String,
    /// Plural resource name, for example `statefulsets`.
    pub resource: String,
    /// The verbs: `get`, `list`, `watch`, `create`, `patch`, `delete`.
    pub verbs: Vec<&'static str>,
}

fn resource_for_kind(kind: &str) -> String {
    KIND_RESOURCES
        .iter()
        .find(|(known, _)| *known == kind)
        .map(|(_, resource)| (*resource).to_string())
        .unwrap_or_else(|| format!("{}s", kind.to_lowercase()))
}

/// The API group of an `apiVersion`: the part before `/`, empty when there is no `/`.
fn group_of(api_version: &str) -> String {
    api_version
        .split_once('/')
        .map(|(group, _)| group.to_string())
        .unwrap_or_default()
}

/// One rule per distinct (group, resource) of `children`, sorted by (group, resource), each
/// with the verbs `get`, `list`, `watch`, `create`, `patch`, `delete`.
///
/// A child without a string `apiVersion` or `kind` is skipped.
pub fn derive_rbac_rules(children: &[Value]) -> Vec<DerivedRule> {
    let pairs: BTreeSet<(String, String)> = children
        .iter()
        .filter_map(|child| {
            let api_version = child.get("apiVersion")?.as_str()?;
            let kind = child.get("kind")?.as_str()?;
            Some((group_of(api_version), resource_for_kind(kind)))
        })
        .collect();
    pairs
        .into_iter()
        .map(|(api_group, resource)| DerivedRule {
            api_group,
            resource,
            verbs: DERIVED_VERBS.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
