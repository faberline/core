//! Kubernetes API implementation of [`AccessStore`] using Server-Side Apply.
//!
//! Only `Role` and `RoleBinding` are ever requested. Every other kind is
//! outside this store.

use std::collections::BTreeMap;

use futures::future::BoxFuture;
use k8s_openapi::api::rbac::v1::{Role, RoleBinding};
use k8s_openapi::NamespaceResourceScope;
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use kube::{Client, Resource};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::domain::access::{AccessObjectKind, AccessStore};
use crate::domain::trust_bundle::StoreError;
use crate::infrastructure::trust_bundle::classify_kube_error;

/// Field manager name used for Server-Side Apply of access Roles and RoleBindings.
pub const ACCESS_FIELD_MANAGER: &str = "service-k8s-access";

/// An [`AccessStore`] that reads, applies, and deletes Roles and RoleBindings
/// in a Kubernetes cluster.
#[derive(Clone)]
pub struct KubernetesAccessStore {
    client: Client,
    field_manager: String,
}

impl KubernetesAccessStore {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            field_manager: ACCESS_FIELD_MANAGER.to_string(),
        }
    }

    pub fn with_field_manager(client: Client, field_manager: impl Into<String>) -> Self {
        Self {
            client,
            field_manager: field_manager.into(),
        }
    }
}

async fn get_labels<K>(
    client: Client,
    namespace: &str,
    name: &str,
) -> Result<Option<BTreeMap<String, String>>, StoreError>
where
    K: Resource<Scope = NamespaceResourceScope> + Clone + std::fmt::Debug + DeserializeOwned,
    K::DynamicType: Default,
{
    let api: Api<K> = Api::namespaced(client, namespace);
    match api.get_opt(name).await {
        Ok(Some(object)) => Ok(Some(object.meta().labels.clone().unwrap_or_default())),
        Ok(None) => Ok(None),
        Err(err) => Err(classify_kube_error(&err)),
    }
}

async fn apply_object<K>(
    client: Client,
    field_manager: &str,
    namespace: &str,
    name: &str,
    object: Value,
) -> Result<(), StoreError>
where
    K: Resource<Scope = NamespaceResourceScope> + Clone + std::fmt::Debug + DeserializeOwned,
    K::DynamicType: Default,
{
    let api: Api<K> = Api::namespaced(client, namespace);
    let params = PatchParams::apply(field_manager).force();
    api.patch(name, &params, &Patch::Apply(object))
        .await
        .map(|_| ())
        .map_err(|err| classify_kube_error(&err))
}

async fn delete_object<K>(client: Client, namespace: &str, name: &str) -> Result<(), StoreError>
where
    K: Resource<Scope = NamespaceResourceScope> + Clone + std::fmt::Debug + DeserializeOwned,
    K::DynamicType: Default,
{
    let api: Api<K> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(api_err)) if api_err.code == 404 => Ok(()),
        Err(err) => Err(classify_kube_error(&err)),
    }
}

impl AccessStore for KubernetesAccessStore {
    fn read<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<BTreeMap<String, String>>, StoreError>> {
        Box::pin(async move {
            match kind {
                AccessObjectKind::Role => {
                    get_labels::<Role>(self.client.clone(), namespace, name).await
                }
                AccessObjectKind::RoleBinding => {
                    get_labels::<RoleBinding>(self.client.clone(), namespace, name).await
                }
            }
        })
    }

    fn apply<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            match kind {
                AccessObjectKind::Role => {
                    apply_object::<Role>(
                        self.client.clone(),
                        &self.field_manager,
                        namespace,
                        name,
                        object,
                    )
                    .await
                }
                AccessObjectKind::RoleBinding => {
                    apply_object::<RoleBinding>(
                        self.client.clone(),
                        &self.field_manager,
                        namespace,
                        name,
                        object,
                    )
                    .await
                }
            }
        })
    }

    fn delete<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            match kind {
                AccessObjectKind::Role => {
                    delete_object::<Role>(self.client.clone(), namespace, name).await
                }
                AccessObjectKind::RoleBinding => {
                    delete_object::<RoleBinding>(self.client.clone(), namespace, name).await
                }
            }
        })
    }
}
