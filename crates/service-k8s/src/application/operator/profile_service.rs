//! `ProfileService<P>`: a generic custom resource that lets the shared controller run any
//! service profile (ADR 0002 D14). The children come from the one renderer.

use std::borrow::Cow;
use std::fmt::Debug;
use std::marker::PhantomData;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::core::{ApiResource, NamespaceResourceScope, ObjectMeta};
use kube::{Client, CustomResourceExt, Resource};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::managed_service::{ManagedService, ReadinessTarget, ReadyFacts, ReconcilePlan};
use crate::domain::condition::{Condition, ConditionFact};
use crate::domain::instance_conditions::{instance_conditions, DomainReadiness, InstanceFacts};
use crate::domain::profile::ServiceProfile;
use crate::infrastructure::manifest::instance_render::{render_operator, InstanceRenderError};
use crate::infrastructure::manifest::{owner_ref, RenderCtx};
use crate::interfaces::service_spec::{crd_for, ServiceSpec};

/// What a profile must add to be run by the controller: its API group, the field-manager and
/// Lease name, the CRD name, and (optionally) its own readiness dimensions.
pub trait ProfileApi: ServiceProfile + Clone + Debug + Send + Sync + 'static {
    /// API group of the profile's CRD, for example `lumen.example`.
    const GROUP: &'static str;
    /// Server-side-apply field manager and leader-election Lease name.
    const MANAGER: &'static str;
    /// Name of the CRD: `<plural>.<group>`, where plural is the lowercase kind plus `s`.
    const CRD_NAME: &'static str;

    /// The app's own readiness dimensions for `spec` (the `DomainReadiness` of the app).
    /// Default: none.
    fn domain_dimensions(
        _spec: &ServiceSpec<Self::Config>,
        _ready_replicas: u32,
    ) -> Vec<ConditionFact> {
        Vec::new()
    }
}

/// The custom resource of profile `P`: metadata, the core spec blocks plus `spec.config`, and
/// an untyped status.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound(
    serialize = "P::Config: Serialize",
    deserialize = "P::Config: DeserializeOwned"
))]
pub struct ProfileService<P: ProfileApi> {
    /// Object metadata.
    #[serde(default)]
    pub metadata: ObjectMeta,
    /// The spec of the instance.
    pub spec: ServiceSpec<P::Config>,
    /// The status as the controller wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Value>,
    #[serde(skip)]
    profile: PhantomData<fn() -> P>,
}

impl<P: ProfileApi> ProfileService<P> {
    /// A resource named `name` with `spec` and no status.
    pub fn new(name: &str, spec: ServiceSpec<P::Config>) -> Self {
        Self {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..ObjectMeta::default()
            },
            spec,
            status: None,
            profile: PhantomData,
        }
    }

    fn api_version() -> String {
        format!("{}/v1", P::GROUP)
    }

    fn plural_name() -> String {
        format!("{}s", P::KIND.to_ascii_lowercase())
    }
}

impl<P: ProfileApi> Resource for ProfileService<P> {
    type DynamicType = ();
    type Scope = NamespaceResourceScope;

    fn kind(_: &()) -> Cow<'_, str> {
        Cow::Borrowed(P::KIND)
    }
    fn group(_: &()) -> Cow<'_, str> {
        Cow::Borrowed(P::GROUP)
    }
    fn version(_: &()) -> Cow<'_, str> {
        Cow::Borrowed("v1")
    }
    fn plural(_: &()) -> Cow<'_, str> {
        Cow::Owned(Self::plural_name())
    }
    fn meta(&self) -> &ObjectMeta {
        &self.metadata
    }
    fn meta_mut(&mut self) -> &mut ObjectMeta {
        &mut self.metadata
    }
}

impl<P: ProfileApi> CustomResourceExt for ProfileService<P>
where
    P::Config: JsonSchema + Serialize + DeserializeOwned,
{
    fn crd() -> CustomResourceDefinition {
        let value = crd_for::<P>(P::GROUP).expect("the profile is valid");
        serde_json::from_value(value).expect("crd_for builds a valid CustomResourceDefinition")
    }
    fn crd_name() -> &'static str {
        P::CRD_NAME
    }
    fn api_resource() -> ApiResource {
        ApiResource::erase::<Self>(&())
    }
    fn shortnames() -> &'static [&'static str] {
        &[]
    }
}

struct Dimensions(Vec<ConditionFact>);

impl DomainReadiness for Dimensions {
    fn dimensions(&self) -> Vec<ConditionFact> {
        self.0.clone()
    }
}

impl<P: ProfileApi> ProfileService<P>
where
    P::Config: Serialize,
{
    /// The one-shard children of this instance, or the render error.
    fn children(&self) -> Result<Vec<Value>, InstanceRenderError> {
        let name = self.metadata.name.clone().unwrap_or_default();
        let ns = self.metadata.namespace.clone().unwrap_or_default();
        let app = P::KIND.to_ascii_lowercase();
        let api_version = Self::api_version();
        let mut cx = RenderCtx::new(&app, P::MANAGER, &api_version, P::KIND, &name, &ns);
        if let Some(uid) = self.metadata.uid.as_deref() {
            cx = cx.with_owner(owner_ref(&api_version, P::KIND, &name, uid));
        }
        render_operator::<P>(&cx, &self.spec)
    }

    /// The name of the StatefulSet that `children` renders, read from the render itself.
    fn statefulset_name(&self) -> Option<String> {
        self.children().ok()?.iter().find_map(|child| {
            (child["kind"] == "StatefulSet")
                .then(|| child["metadata"]["name"].as_str().map(str::to_string))
                .flatten()
        })
    }
}

impl<P: ProfileApi> ManagedService for ProfileService<P>
where
    P::Config: Clone + Debug + Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static,
{
    const MANAGER: &'static str = <P as ProfileApi>::MANAGER;

    fn render(&self) -> Vec<Value> {
        self.children().unwrap_or_default()
    }

    async fn reconcile_plan(&self, _client: Client) -> anyhow::Result<ReconcilePlan> {
        let children = self.children()?;
        Ok(ReconcilePlan::new(children, Value::Null))
    }

    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        self.statefulset_name()
            .map(|name| vec![ReadinessTarget::new("StatefulSet", name)])
            .unwrap_or_default()
    }

    fn status_patch(&self, ready: &ReadyFacts) -> Value {
        let ready_replicas = self
            .statefulset_name()
            .map(|name| ready.get(&name))
            .unwrap_or(0);
        json!({ "status": { "readyReplicas": ready_replicas } })
    }

    fn conditions(&self, ready: &ReadyFacts, _context: &Value) -> Vec<ConditionFact> {
        let ready_replicas = self
            .statefulset_name()
            .map(|name| u32::try_from(ready.get(&name).max(0)).unwrap_or(u32::MAX))
            .unwrap_or(0);
        let facts = InstanceFacts {
            ready_replicas,
            desired_replicas: self.spec.replicas.replicas_per_shard,
            storage_healthy: true,
            backup_configured: self.spec.backup.schedule.is_some(),
            last_backup_succeeded: None,
        };
        let domain = Dimensions(P::domain_dimensions(&self.spec, ready_replicas));
        instance_conditions(&facts, &domain)
    }

    fn observed_conditions(&self) -> Vec<Condition> {
        self.status
            .as_ref()
            .and_then(|status| status.get("conditions"))
            .and_then(|conditions| serde_json::from_value(conditions.clone()).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
