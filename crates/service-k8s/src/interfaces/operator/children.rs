//! Generic child-object adapters: dynamic server-side apply, owner-checked prune,
//! cluster-scoped child finalizers and ready-replica reads.

use std::collections::BTreeMap;

use kube::api::{
    Api, ApiResource, DeleteParams, DynamicObject, ListParams, Patch, PatchParams, Preconditions,
};
use kube::{Client, ResourceExt};
use serde_json::{json, Value};

use super::Error;
use crate::domain::retention::INSTANCE_UID_LABEL;
use crate::service::{self, ManagedService};

/// Plural for a kind. Covers the kinds the toolkit + services render; falls back
/// to the naive `lower(kind)+"s"`.
pub(super) fn plural_for(kind: &str) -> String {
    match kind {
        "Deployment" => "deployments",
        "Service" => "services",
        "ConfigMap" => "configmaps",
        "ServiceAccount" => "serviceaccounts",
        "HorizontalPodAutoscaler" => "horizontalpodautoscalers",
        "PodDisruptionBudget" => "poddisruptionbudgets",
        "StatefulSet" => "statefulsets",
        "ServiceMonitor" => "servicemonitors",
        "PrometheusRule" => "prometheusrules",
        "ClusterRoleBinding" => "clusterrolebindings",
        "FQDNNetworkPolicy" => "fqdnnetworkpolicies",
        // The fallback would yield `networkpolicys` — a plural no apiserver
        // serves, so every apply of a rendered NetworkPolicy would 404 at
        // runtime with nothing failing at build time (#2603).
        "NetworkPolicy" => "networkpolicies",
        other => return format!("{}s", other.to_lowercase()),
    }
    .to_string()
}

/// Build the `ApiResource` (GVK + plural) for a dynamic apply.
pub(super) fn api_resource(api_version: &str, kind: &str) -> ApiResource {
    let (group, version) = match api_version.split_once('/') {
        Some((g, v)) => (g.to_string(), v.to_string()),
        None => (String::new(), api_version.to_string()),
    };
    ApiResource {
        group,
        version,
        api_version: api_version.to_string(),
        kind: kind.to_string(),
        plural: plural_for(kind),
    }
}

/// Server-side-apply one rendered object into `ns` as field manager `manager`.
pub(super) async fn apply_object(
    client: &Client,
    ns: &str,
    manager: &str,
    value: Value,
) -> Result<(), Error> {
    let api_version = value["apiVersion"]
        .as_str()
        .ok_or(Error::Missing("apiVersion"))?
        .to_string();
    let kind = value["kind"]
        .as_str()
        .ok_or(Error::Missing("kind"))?
        .to_string();
    let name = value["metadata"]["name"]
        .as_str()
        .ok_or(Error::Missing("metadata.name"))?
        .to_string();

    // A child that names its own namespace (#663) is applied there. Otherwise
    // the owner namespace is used, as before.
    let child_ns = value["metadata"]["namespace"]
        .as_str()
        .unwrap_or(ns)
        .to_string();
    let ar = api_resource(&api_version, &kind);
    let obj: DynamicObject = serde_json::from_value(value)?;
    let api: Api<DynamicObject> = if kind == "ClusterRoleBinding" {
        Api::all_with(client.clone(), &ar)
    } else {
        Api::namespaced_with(client.clone(), &child_ns, &ar)
    };
    api.patch(
        &name,
        &PatchParams::apply(manager).force(),
        &Patch::Apply(&obj),
    )
    .await?;
    tracing::debug!(%kind, %name, "applied");
    Ok(())
}

/// How one prune target's pass ended (#3079).
///
/// `prune_object` used to answer `Result<(), Error>`, which folded three
/// different endings into one `Ok` and a fourth into an `Error` that aborted
/// the whole reconcile. The one that does not belong there is an API the
/// cluster does not serve: nothing was removed, nothing can be, and no restart
/// or retry of the operator changes that — so it has to reach the caller as a
/// value it can report, not as an error that skips the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PruneOutcome {
    /// The target is gone: deleted just now, already absent, or lost the delete
    /// race against the CR's own garbage collection.
    Settled,
    /// An object exists at the target's name and this CR does not control it,
    /// so it was left alone. Already logged; the CR claims nothing about an
    /// object it never made.
    Foreign,
    /// The cluster does not serve the target's API at all. Nothing could be
    /// observed and nothing was removed.
    Unavailable,
}

/// Delete one object the service no longer renders (#2603) — but only if this
/// CR owns it.
///
/// The ownership re-check is the whole safety story. `prunes()` hands back a
/// name, and a name in a namespace is not proof of authorship: another
/// controller, a Helm chart, or a human could have created a NetworkPolicy at
/// exactly the CR's name. Matching the live object's controller
/// `ownerReference` UID against the CR's own UID is proof, because only the
/// apiserver writes that link and only for objects we submitted with it.
///
/// Absent object → no-op, and a 404 racing the delete → success, so a prune
/// that runs on every requeue converges once and then costs one GET.
///
/// The answer is a [`PruneOutcome`] rather than `()` because one of the ways a
/// prune can end is neither success nor an operator error, and the caller has
/// to be able to tell (#3079).
pub(super) async fn prune_object(
    client: &Client,
    ns: &str,
    owner_uid: &str,
    target: &service::PruneTarget,
) -> Result<PruneOutcome, Error> {
    let ar = api_resource(target.api_version(), target.kind());
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &ar);
    // `get` and not `get_opt`: kube 5 also maps a bare 404 (no reason, or a
    // reason it does not know) to `Ok(None)`, which would report an unserved
    // API as settled. Only the `NotFound` reason means the object is absent.
    let live = match api.get(target.name()).await {
        Ok(live) => live,
        Err(kube::Error::Api(e)) if e.reason == "NotFound" => {
            return Ok(PruneOutcome::Settled);
        }
        // A 404 that reaches this arm is not an absent object. It is one
        // the apiserver mux answered because nothing routed the request: the
        // cluster does not serve this API. Restarting the operator cannot fix
        // that, and neither can retrying faster, so it is reported as a value
        // and the reconcile converges everything else.
        //
        // The discriminator is the status code, never the reason string.
        // kube-client reconstructs `"Failed to parse error data"` for a body it
        // could not parse, which is an implementation detail of *this* client
        // and not a wire contract — a cluster that returns a parseable 404
        // carrying some other reason is in the same situation and has to be
        // classified the same way.
        //
        // Every other status deliberately stays fatal: a 403 is a missing RBAC
        // grant and a 5xx is a backend that should already be alerting. Both
        // are actionable by a human, and both belong in
        // `reconcile_errors_total` rather than in a status condition.
        Err(kube::Error::Api(e)) if e.code == 404 => {
            tracing::warn!(
                api_version = %target.api_version(), kind = %target.kind(),
                name = %target.name(), namespace = %ns,
                "prune: this cluster does not serve the target's API — nothing \
                 was removed; reporting it on the CR and retrying next pass"
            );
            return Ok(PruneOutcome::Unavailable);
        }
        Err(err) => return Err(err.into()),
    };
    let owned = live
        .metadata
        .owner_references
        .iter()
        .flatten()
        .any(|r| r.uid == owner_uid && r.controller.unwrap_or(false));
    if !owned {
        tracing::warn!(
            kind = %target.kind(), name = %target.name(), namespace = %ns,
            "prune: an object of this kind exists at the CR's name but is not \
             controller-owned by it — leaving it alone"
        );
        return Ok(PruneOutcome::Foreign);
    }
    match api.delete(target.name(), &Default::default()).await {
        Ok(_) => {
            tracing::info!(
                kind = %target.kind(), name = %target.name(), namespace = %ns,
                "prune: deleted a child the spec no longer asks for"
            );
            Ok(PruneOutcome::Settled)
        }
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(PruneOutcome::Settled),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn cluster_children_finalizer<S: ManagedService>() -> String {
    format!("service-k8s.axiom.dev/{}-cluster-children", S::MANAGER)
}

pub(super) fn other_namespace_finalizer<S: ManagedService>() -> String {
    format!(
        "service-k8s.axiom.dev/{}-other-namespace-children",
        S::MANAGER
    )
}

pub(super) fn retention_finalizer<S: ManagedService>() -> String {
    format!("service-k8s.axiom.dev/{}-data-retention", S::MANAGER)
}

/// Scale one workload to 0 replicas. A workload that is already gone is fine.
pub(super) async fn scale_workload_to_zero(
    client: &Client,
    ns: &str,
    kind: &str,
    name: &str,
) -> Result<(), Error> {
    if kind != "StatefulSet" && kind != "Deployment" {
        return Err(Error::Plan(format!("data retention cannot scale a {kind}")));
    }
    let ar = api_resource("apps/v1", kind);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &ar);
    match api
        .patch(
            name,
            &PatchParams::default(),
            &Patch::Merge(json!({"spec": {"replicas": 0}})),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PvcCleanup {
    Done,
    Pending,
}

/// Delete the PVCs of one instance on a `Delete` policy (#519). Only PVCs whose
/// labels hold this instance's UID are deleted; a server-side selector is not
/// proof, so every object is checked again. A terminating PVC is waited for.
pub(super) async fn delete_owned_pvcs(
    client: &Client,
    ns: &str,
    owner_uid: &str,
    selector: &BTreeMap<String, String>,
) -> Result<PvcCleanup, Error> {
    let ar = api_resource("v1", "PersistentVolumeClaim");
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &ar);
    let mut wanted = selector.clone();
    wanted.insert(INSTANCE_UID_LABEL.to_string(), owner_uid.to_string());
    let selector_string = wanted
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    let list = api
        .list(&ListParams::default().labels(&selector_string))
        .await?;

    let mut pending = false;
    for pvc in list.items {
        let labels_match = wanted.iter().all(|(key, value)| {
            pvc.metadata
                .labels
                .as_ref()
                .and_then(|labels| labels.get(key))
                == Some(value)
        });
        if !labels_match {
            continue;
        }
        if pvc.metadata.deletion_timestamp.is_some() {
            pending = true;
            continue;
        }
        let name = pvc.name_any();
        let Some(uid) = pvc.metadata.uid.clone() else {
            return Err(Error::ClusterOwnership {
                kind: "PersistentVolumeClaim".to_string(),
                name,
                reason: "live object has no UID for a safe delete precondition".to_string(),
            });
        };
        let delete_params = DeleteParams {
            preconditions: Some(Preconditions {
                uid: Some(uid),
                resource_version: pvc.metadata.resource_version.clone(),
            }),
            ..DeleteParams::default()
        };
        match api.delete(&name, &delete_params).await {
            Ok(_) => {}
            Err(kube::Error::Api(error)) if error.code == 404 => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(if pending {
        PvcCleanup::Pending
    } else {
        PvcCleanup::Done
    })
}

pub(super) async fn patch_finalizers<S: ManagedService>(
    api: &Api<S>,
    obj: &S,
    finalizers: Vec<String>,
) -> Result<(), Error> {
    let mut metadata = json!({"finalizers": finalizers});
    if let Some(resource_version) = obj.meta().resource_version.as_deref() {
        metadata["resourceVersion"] = Value::String(resource_version.to_string());
    }
    api.patch(
        &obj.name_any(),
        &PatchParams::default(),
        &Patch::Merge(json!({"metadata": metadata})),
    )
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClusterPruneOutcome {
    Absent,
    DeleteRequested,
}

/// Remove one cluster-scoped child only when labels and the SSA manager both
/// prove that this controller created it for this CR.
pub(super) async fn prune_cluster_scoped_object(
    client: &Client,
    manager: &str,
    target: &service::ClusterScopedChild,
) -> Result<ClusterPruneOutcome, Error> {
    let ar = api_resource(target.api_version(), target.kind());
    let api: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
    let Some(live) = api.get_opt(target.name()).await? else {
        return Ok(ClusterPruneOutcome::Absent);
    };

    let labels_match = target.expected_labels().iter().all(|(key, value)| {
        live.metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get(key))
            == Some(value)
    });
    if !labels_match {
        return Err(Error::ClusterOwnership {
            kind: target.kind().to_string(),
            name: target.name().to_string(),
            reason: "expected owner labels do not match the live object".to_string(),
        });
    }
    let managed = live
        .metadata
        .managed_fields
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|entry| entry.manager.as_deref() == Some(manager));
    if !managed {
        return Err(Error::ClusterOwnership {
            kind: target.kind().to_string(),
            name: target.name().to_string(),
            reason: format!("managedFields does not contain field manager {manager}"),
        });
    }

    let Some(uid) = live.metadata.uid.clone() else {
        return Err(Error::ClusterOwnership {
            kind: target.kind().to_string(),
            name: target.name().to_string(),
            reason: "live object has no UID for a safe delete precondition".to_string(),
        });
    };
    let delete_params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: Some(uid),
            resource_version: live.metadata.resource_version.clone(),
        }),
        ..DeleteParams::default()
    };

    match api.delete(target.name(), &delete_params).await {
        Ok(_) => Ok(ClusterPruneOutcome::DeleteRequested),
        Err(kube::Error::Api(error)) if error.code == 404 => Ok(ClusterPruneOutcome::Absent),
        Err(error) => Err(error.into()),
    }
}

/// Place one rendered child that names a namespace other than the owner's
/// (#663). A child in the owner namespace is returned unchanged. Any other
/// namespace must match a `desired` entry of `declared`, or the plan is
/// refused. The owner references are removed, because a namespaced owner
/// cannot own an object in another namespace. The owner UID label and the
/// entry's expected labels are stamped on the child.
pub(super) fn place_child(
    mut child: Value,
    owner_ns: &str,
    owner_uid: &str,
    declared: &[service::OtherNamespaceChild],
) -> Result<Value, Error> {
    let child_ns = child["metadata"]["namespace"].as_str().map(str::to_string);
    let Some(child_ns) = child_ns.filter(|ns| ns != owner_ns) else {
        return Ok(child);
    };
    let api_version = child["apiVersion"].as_str().unwrap_or_default().to_string();
    let kind = child["kind"].as_str().unwrap_or_default().to_string();
    let name = child["metadata"]["name"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let Some(entry) = declared.iter().find(|entry| {
        entry.desired()
            && entry.api_version() == api_version
            && entry.kind() == kind
            && entry.namespace() == child_ns
            && entry.name() == name
    }) else {
        return Err(Error::Plan(format!(
            "child {kind}/{name} names namespace {child_ns} but the service does not declare it as desired in other_namespace_children"
        )));
    };

    let metadata = child["metadata"]
        .as_object_mut()
        .ok_or(Error::Missing("metadata"))?;
    metadata.remove("ownerReferences");
    let labels = metadata
        .entry("labels")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(Error::Missing("metadata.labels"))?;
    labels.insert(INSTANCE_UID_LABEL.to_string(), json!(owner_uid));
    for (key, value) in entry.expected_labels() {
        labels.insert(key.clone(), json!(value));
    }
    Ok(child)
}

/// Delete one child that lives in another namespace (#663), but only when
/// the live object proves full ownership. Every refusal is an
/// [`Error::NamespacedOwnership`]. Its checks are the expected labels, the
/// owner UID label, the field manager, and a UID for the delete precondition.
pub(super) async fn prune_other_namespace_object(
    client: &Client,
    manager: &str,
    owner_uid: &str,
    target: &service::OtherNamespaceChild,
) -> Result<ClusterPruneOutcome, Error> {
    let ar = api_resource(target.api_version(), target.kind());
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), target.namespace(), &ar);
    let Some(live) = api.get_opt(target.name()).await? else {
        return Ok(ClusterPruneOutcome::Absent);
    };
    let refuse = |reason: &str| Error::NamespacedOwnership {
        kind: target.kind().to_string(),
        namespace: target.namespace().to_string(),
        name: target.name().to_string(),
        reason: reason.to_string(),
    };

    let labels = live.metadata.labels.as_ref();
    let labels_match = target
        .expected_labels()
        .iter()
        .all(|(key, value)| labels.and_then(|labels| labels.get(key)) == Some(value));
    if !labels_match {
        return Err(refuse("expected owner labels do not match the live object"));
    }
    let uid_matches = labels
        .and_then(|labels| labels.get(INSTANCE_UID_LABEL))
        .map(String::as_str)
        == Some(owner_uid);
    if !uid_matches {
        return Err(refuse("owner uid label does not match"));
    }
    let managed = live
        .metadata
        .managed_fields
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|entry| entry.manager.as_deref() == Some(manager));
    if !managed {
        return Err(refuse(&format!(
            "managedFields does not contain field manager {manager}"
        )));
    }

    let Some(uid) = live.metadata.uid.clone() else {
        return Err(refuse(
            "live object has no UID for a safe delete precondition",
        ));
    };
    let delete_params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: Some(uid),
            resource_version: live.metadata.resource_version.clone(),
        }),
        ..DeleteParams::default()
    };

    match api.delete(target.name(), &delete_params).await {
        Ok(_) => Ok(ClusterPruneOutcome::DeleteRequested),
        Err(kube::Error::Api(error)) if error.code == 404 => Ok(ClusterPruneOutcome::Absent),
        Err(error) => Err(error.into()),
    }
}

fn workload_ready_replicas(kind: &str, data: &Value) -> i64 {
    let field = match kind {
        "DaemonSet" => "numberReady",
        _ => "readyReplicas",
    };
    data["status"][field].as_i64().unwrap_or(0)
}

/// Read the workload kind's native ready count, or 0 if absent.
pub(super) async fn ready_replicas(
    client: &Client,
    ns: &str,
    kind: &str,
    name: &str,
) -> Result<i64, Error> {
    let ar = api_resource("apps/v1", kind);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &ar);
    Ok(api
        .get_opt(name)
        .await?
        .map(|o| workload_ready_replicas(kind, &o.data))
        .unwrap_or(0))
}
