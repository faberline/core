//! The [`ManagedService`] trait a service implements, and the plan, readiness
//! and child types it hands the shared controller.

#[cfg(feature = "controller")]
use std::collections::{BTreeMap, HashMap};
#[cfg(feature = "controller")]
use std::fmt::Debug;
#[cfg(feature = "controller")]
use std::future::Future;

#[cfg(feature = "controller")]
use kube::core::NamespaceResourceScope;
#[cfg(feature = "controller")]
use kube::{Client, CustomResourceExt, Resource};
#[cfg(feature = "controller")]
use serde::de::DeserializeOwned;

#[cfg(doc)]
use crate::domain::condition::project;
use crate::domain::condition::{Condition, ConditionFact};

/// A workload to poll for `.status.readyReplicas` during reconcile.
#[cfg(feature = "controller")]
pub struct ReadinessTarget {
    kind: &'static str,
    name: String,
}

#[cfg(feature = "controller")]
impl ReadinessTarget {
    /// Poll the workload of `kind` (`StatefulSet`, `Deployment` or
    /// `DaemonSet`) named `name` in the CR's namespace.
    pub fn new(kind: &'static str, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
        }
    }

    /// The workload kind.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The workload name; also the key of its count in [`ReadyFacts`].
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Observed readiness handed to [`ManagedService::status_patch`]
/// (workload name → `readyReplicas`).
#[cfg(feature = "controller")]
pub struct ReadyFacts {
    ready: HashMap<String, i64>,
}

#[cfg(feature = "controller")]
impl ReadyFacts {
    /// Readiness observed as workload name → `readyReplicas`.
    pub fn new(ready: HashMap<String, i64>) -> Self {
        Self { ready }
    }

    /// Ready replicas for `name`, or 0 if the workload was absent.
    pub fn get(&self, name: &str) -> i64 {
        self.ready.get(name).copied().unwrap_or(0)
    }

    /// Every observed count, by workload name.
    pub fn ready(&self) -> &HashMap<String, i64> {
        &self.ready
    }
}

/// One service-specific planning result consumed by the shared controller.
/// `context` is opaque to service-k8s and is handed back to the same service
/// only after children have been applied and readiness has been observed.
#[cfg(feature = "controller")]
pub struct ReconcilePlan {
    children: Vec<serde_json::Value>,
    context: serde_json::Value,
}

#[cfg(feature = "controller")]
impl ReconcilePlan {
    /// The children to server-side-apply, and the context handed back to
    /// [`ManagedService::status_patch_with_context`] and
    /// [`ManagedService::conditions`].
    pub fn new(children: Vec<serde_json::Value>, context: serde_json::Value) -> Self {
        Self { children, context }
    }

    /// The children to server-side-apply.
    pub fn children(&self) -> &[serde_json::Value] {
        &self.children
    }

    /// The service's opaque context.
    pub fn context(&self) -> &serde_json::Value {
        &self.context
    }

    /// The children and the context, moved out.
    pub fn into_parts(self) -> (Vec<serde_json::Value>, serde_json::Value) {
        (self.children, self.context)
    }
}

/// One service's contribution to the shared operator. Implemented on the CRD
/// root type (e.g. lumen's `Lumen`). The [`crate::controller`] is generic over
/// `S`, so the watch/apply/lease loop is written once.
#[cfg(feature = "controller")]
pub trait ManagedService:
    Resource<DynamicType = (), Scope = NamespaceResourceScope>
    + CustomResourceExt
    + Clone
    + Debug
    + DeserializeOwned
    + Send
    + Sync
    + 'static
{
    /// Server-side-apply field manager **and** the leader-election Lease name.
    /// Per-service so two operators never collide on the same Lease.
    const MANAGER: &'static str;

    /// Pure render: the spec (+ metadata via `ResourceExt`) → the child objects
    /// to server-side-apply. No I/O.
    fn render(&self) -> Vec<serde_json::Value>;

    /// Optional async pre-apply planning hook. Existing services keep the pure
    /// render behavior; services with external admission can inspect Kubernetes
    /// or remote state and carry contextual facts into status projection.
    fn reconcile_plan(
        &self,
        _client: Client,
    ) -> impl Future<Output = anyhow::Result<ReconcilePlan>> + Send {
        let children = self.render();
        async move { Ok(ReconcilePlan::new(children, serde_json::Value::Null)) }
    }

    /// The workloads whose `.status.readyReplicas` feed [`Self::status_patch`].
    fn readiness_targets(&self) -> Vec<ReadinessTarget>;

    /// The `{ "status": { … } }` subresource patch given observed readiness.
    fn status_patch(&self, ready: &ReadyFacts) -> serde_json::Value;

    /// Context-aware status projection paired with [`Self::reconcile_plan`].
    /// Defaults to the original readiness-only contract.
    fn status_patch_with_context(
        &self,
        ready: &ReadyFacts,
        _context: &serde_json::Value,
    ) -> serde_json::Value {
        self.status_patch(ready)
    }

    /// The `status.conditions[]` this service reports for the observed state,
    /// in the order they should appear (#2601).
    ///
    /// Clock-free by construction: [`project`] stamps `lastTransitionTime` with
    /// a time the controller injects, so this stays a pure function of spec +
    /// observed facts and its tests stay deterministic.
    ///
    /// Defaults to none, so a service that has not adopted conditions keeps its
    /// existing status shape byte-for-byte.
    fn conditions(&self, _ready: &ReadyFacts, _context: &serde_json::Value) -> Vec<ConditionFact> {
        Vec::new()
    }

    /// The conditions already persisted on this object's status (#2601).
    ///
    /// The controller writes status with `Patch::Merge`, which replaces arrays
    /// wholesale, so transition times cannot survive server-side — they have to
    /// be read back off the watched object (which carries `.status`) and carried
    /// forward explicitly by [`project`].
    fn observed_conditions(&self) -> Vec<Condition> {
        Vec::new()
    }

    /// Children this service rendered under a previous spec but no longer
    /// wants to exist (#2603).
    ///
    /// Server-side apply reconciles *fields*, never object lifetime: a child
    /// that drops out of [`Self::render`] simply stops being updated and keeps
    /// running until the owning CR is deleted. For most children that is
    /// harmless. For one whose entire purpose is to enforce something — a
    /// NetworkPolicy — it makes the toggle a one-way door: turning it on takes
    /// effect, turning it off does not, and the operator silently keeps
    /// enforcing a posture the spec no longer asks for. Naming the object here
    /// closes that door.
    ///
    /// Only ever name objects this CR owns. The controller re-checks ownership
    /// against the live object before deleting, so a target that turns out to
    /// belong to something else is inert rather than destructive — but the
    /// check is a safety net, not a license to guess.
    ///
    /// Defaults to none, so a service that has not adopted pruning keeps its
    /// existing behavior exactly.
    fn prunes(&self) -> Vec<PruneTarget> {
        Vec::new()
    }

    /// Cluster-scoped children this namespaced CR may create.
    ///
    /// Kubernetes forbids a cluster-scoped object from carrying an owner
    /// reference to a namespaced CR. Services that render such an object must
    /// therefore declare it here. The shared controller installs a finalizer
    /// before the object is applied, removes an undesired object during normal
    /// reconcile, and removes every declared object before CR deletion.
    ///
    /// `expected_labels` and the server-side-apply manager are both checked
    /// before deletion. A name alone is never accepted as ownership proof.
    fn cluster_scoped_children(&self) -> Vec<ClusterScopedChild> {
        Vec::new()
    }
}

/// One object a service no longer renders and wants removed (#2603).
///
/// Namespace is not a field: the controller prunes in the CR's own namespace,
/// which is the only place [`ManagedService::render`] can place children.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(feature = "controller")]
pub struct PruneTarget {
    api_version: &'static str,
    kind: &'static str,
    name: String,
}

#[cfg(feature = "controller")]
impl PruneTarget {
    /// The object of `api_version` and `kind` named `name` in the CR's
    /// namespace.
    pub fn new(api_version: &'static str, kind: &'static str, name: impl Into<String>) -> Self {
        Self {
            api_version,
            kind,
            name: name.into(),
        }
    }

    /// The object's `apiVersion`, for example `networking.k8s.io/v1`.
    pub fn api_version(&self) -> &'static str {
        self.api_version
    }

    /// The object's kind.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The object's name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// One cluster-scoped child whose lifetime follows a namespaced CR.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(feature = "controller")]
pub struct ClusterScopedChild {
    api_version: &'static str,
    kind: &'static str,
    name: String,
    expected_labels: BTreeMap<String, String>,
    desired: bool,
}

#[cfg(feature = "controller")]
impl ClusterScopedChild {
    /// The cluster-scoped object of `api_version` and `kind` named `name`.
    /// `desired` is `true` when the current spec renders the child, and
    /// `false` when a prior version may have rendered it and the controller
    /// must remove it. It starts with no expected labels.
    pub fn new(
        api_version: &'static str,
        kind: &'static str,
        name: impl Into<String>,
        desired: bool,
    ) -> Self {
        Self {
            api_version,
            kind,
            name: name.into(),
            expected_labels: BTreeMap::new(),
            desired,
        }
    }

    /// Adds labels the live object must carry before the controller deletes
    /// it.
    pub fn with_expected_labels(mut self, labels: BTreeMap<String, String>) -> Self {
        self.expected_labels.extend(labels);
        self
    }

    /// The object's `apiVersion`.
    pub fn api_version(&self) -> &'static str {
        self.api_version
    }

    /// The object's kind.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The object's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The labels the live object must carry before it is deleted.
    pub fn expected_labels(&self) -> &BTreeMap<String, String> {
        &self.expected_labels
    }

    /// Whether the current spec renders the child.
    pub fn desired(&self) -> bool {
        self.desired
    }
}

#[cfg(all(test, feature = "controller"))]
mod tests;
