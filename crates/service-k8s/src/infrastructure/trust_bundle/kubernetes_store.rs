//! Kubernetes API implementation of [`ConfigMapStore`] using Server-Side Apply.

use futures::future::BoxFuture;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use kube::Client;
use serde_json::Value;

use crate::domain::trust_bundle::{ConfigMapStore, StoreError, StoreErrorKind, StoredConfigMap};

/// Field manager name used for Server-Side Apply of trust bundle ConfigMaps.
pub const TRUST_BUNDLE_FIELD_MANAGER: &str = "service-k8s-trust-bundle";

/// Classify a `kube::Error` into a typed `StoreError`.
pub fn classify_kube_error(err: &kube::Error) -> StoreError {
    match err {
        kube::Error::Api(api_err) => match api_err.code {
            403 => StoreError::forbidden(&api_err.message),
            404 => StoreError::not_found(&api_err.message),
            409 => StoreError::conflict(&api_err.message),
            code if code >= 500 => StoreError::new(StoreErrorKind::Unavailable, &api_err.message),
            code => StoreError::new(StoreErrorKind::Other(code), &api_err.message),
        },
        _ => StoreError::unavailable(err.to_string()),
    }
}

/// A [`ConfigMapStore`] that reads, applies, and deletes ConfigMaps in a Kubernetes cluster.
#[derive(Clone)]
pub struct KubernetesConfigMapStore {
    client: Client,
    field_manager: String,
}

impl KubernetesConfigMapStore {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            field_manager: TRUST_BUNDLE_FIELD_MANAGER.to_string(),
        }
    }

    pub fn with_field_manager(client: Client, field_manager: impl Into<String>) -> Self {
        Self {
            client,
            field_manager: field_manager.into(),
        }
    }
}

impl ConfigMapStore for KubernetesConfigMapStore {
    fn read<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<StoredConfigMap>, StoreError>> {
        Box::pin(async move {
            let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
            match api.get_opt(name).await {
                Ok(Some(cm)) => {
                    let data = cm.data.unwrap_or_default();
                    let labels = cm.metadata.labels.unwrap_or_default();
                    let annotations = cm.metadata.annotations.unwrap_or_default();
                    Ok(Some(StoredConfigMap {
                        data,
                        labels,
                        annotations,
                    }))
                }
                Ok(None) => Ok(None),
                Err(err) => Err(classify_kube_error(&err)),
            }
        })
    }

    fn apply<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
            let params = PatchParams::apply(&self.field_manager).force();
            api.patch(name, &params, &Patch::Apply(object))
                .await
                .map(|_| ())
                .map_err(|err| classify_kube_error(&err))
        })
    }

    fn delete<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
            match api.delete(name, &DeleteParams::default()).await {
                Ok(_) => Ok(()),
                Err(kube::Error::Api(api_err)) if api_err.code == 404 => Ok(()),
                Err(err) => Err(classify_kube_error(&err)),
            }
        })
    }
}
