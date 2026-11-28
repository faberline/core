//! Kube adapter of the `FleetCluster` port (#678): reads, applies, adopts,
//! deletes and probes Fleet children through a `kube::Client`.

use kube::api::{
    Api, ApiResource, DeleteParams, DynamicObject, GroupVersionKind, ListParams, Patch, PatchParams,
};
use kube::{Client, ResourceExt};
use serde_json::{json, Value};

use crate::application::fleet::{FleetCluster, FleetClusterError};
use crate::domain::fleet::{ChildKey, ObservedChild, Ownership};

/// Label that carries the owner of a child: the uid of the Fleet object.
pub const FLEET_OWNER_LABEL: &str = "faberline.dev/fleet-owner";
/// Annotation that carries the desired-state hash of a child.
pub const FLEET_HASH_ANNOTATION: &str = "faberline.dev/desired-hash";

/// The kube adapter of [`FleetCluster`].
///
/// Call the port only from a thread that is not an async worker, for example
/// inside `tokio::task::spawn_blocking`. `Handle::block_on` panics inside an
/// async context.
pub struct KubeFleetCluster {
    client: Client,
    handle: tokio::runtime::Handle,
    /// `None` lists owned children cluster-wide (needs a ClusterRole).
    /// `Some(list)` lists only in `list` and in the namespaces of the keys
    /// (a namespaced Role is enough).
    namespaces: Option<Vec<String>>,
}

impl KubeFleetCluster {
    /// Create the adapter. See the struct docs for the `namespaces` rule.
    pub fn new(
        client: Client,
        handle: tokio::runtime::Handle,
        namespaces: Option<Vec<String>>,
    ) -> Self {
        Self {
            client,
            handle,
            namespaces,
        }
    }

    fn api_for(&self, key: &ChildKey) -> Api<DynamicObject> {
        let resource = api_resource(&key.api_version, &key.kind);
        match &key.namespace {
            Some(namespace) => Api::namespaced_with(self.client.clone(), namespace, &resource),
            None => Api::all_with(self.client.clone(), &resource),
        }
    }
}

/// The resource of a child kind, with the plural the apiserver serves.
fn api_resource(api_version: &str, kind: &str) -> ApiResource {
    let (group, version) = match api_version.split_once('/') {
        Some((group, version)) => (group, version),
        None => ("", api_version),
    };
    ApiResource::from_gvk(&GroupVersionKind::gvk(group, version, kind))
}

fn err(verb: &str, key: &ChildKey, error: kube::Error) -> FleetClusterError {
    FleetClusterError(format!("{verb} {key}: {error}"))
}

impl FleetCluster for KubeFleetCluster {
    fn observed(
        &mut self,
        owner: &str,
        keys: &[ChildKey],
    ) -> Result<Vec<ObservedChild>, FleetClusterError> {
        let mut result = Vec::new();
        for key in keys {
            let found = self
                .handle
                .block_on(self.api_for(key).get_opt(&key.name))
                .map_err(|error| err("observe", key, error))?;
            if let Some(object) = found {
                let label = object.labels().get(FLEET_OWNER_LABEL).map(String::as_str);
                let hash = object.annotations().get(FLEET_HASH_ANNOTATION).cloned();
                result.push(ObservedChild::new(
                    key.clone(),
                    Ownership::from_owner(label, owner),
                    hash,
                ));
            }
        }

        // One representative key per kind that has a namespace; keys without a
        // namespace are not listed.
        let mut kinds: Vec<&ChildKey> = keys.iter().filter(|k| k.namespace.is_some()).collect();
        kinds.sort_by(|a, b| (&a.api_version, &a.kind).cmp(&(&b.api_version, &b.kind)));
        kinds.dedup_by(|a, b| a.api_version == b.api_version && a.kind == b.kind);

        let mut namespaces: Vec<String> = match &self.namespaces {
            Some(list) => list
                .iter()
                .cloned()
                .chain(keys.iter().filter_map(|k| k.namespace.clone()))
                .collect(),
            None => Vec::new(),
        };
        namespaces.sort();
        namespaces.dedup();

        let selector = ListParams::default().labels(&format!("{FLEET_OWNER_LABEL}={owner}"));
        let mut extras: Vec<ObservedChild> = Vec::new();
        for representative in kinds {
            let resource = api_resource(&representative.api_version, &representative.kind);
            let lists: Vec<Api<DynamicObject>> = match &self.namespaces {
                Some(_) => namespaces
                    .iter()
                    .map(|namespace| {
                        Api::namespaced_with(self.client.clone(), namespace, &resource)
                    })
                    .collect(),
                None => vec![Api::all_with(self.client.clone(), &resource)],
            };
            for api in lists {
                let listed = self
                    .handle
                    .block_on(api.list(&selector))
                    .map_err(|error| err("observe", representative, error))?;
                for object in listed.items {
                    let child_key = ChildKey::new(
                        representative.api_version.clone(),
                        representative.kind.clone(),
                        object.namespace(),
                        object.name_any(),
                    );
                    if result.iter().any(|c| c.key == child_key)
                        || extras.iter().any(|c| c.key == child_key)
                    {
                        continue;
                    }
                    let hash = object.annotations().get(FLEET_HASH_ANNOTATION).cloned();
                    extras.push(ObservedChild::new(child_key, Ownership::Owned, hash));
                }
            }
        }
        extras.sort_by(|a, b| a.key.cmp(&b.key));
        result.extend(extras);
        Ok(result)
    }

    fn apply(
        &mut self,
        key: &ChildKey,
        spec: &Value,
        desired_hash: &str,
        owner: &str,
        manager: &str,
    ) -> Result<(), FleetClusterError> {
        let mut metadata = serde_json::Map::new();
        metadata.insert("name".into(), json!(key.name));
        if let Some(namespace) = &key.namespace {
            metadata.insert("namespace".into(), json!(namespace));
        }
        metadata.insert("labels".into(), json!({ (FLEET_OWNER_LABEL): owner }));
        metadata.insert(
            "annotations".into(),
            json!({ (FLEET_HASH_ANNOTATION): desired_hash }),
        );
        let object = json!({
            "apiVersion": key.api_version,
            "kind": key.kind,
            "metadata": Value::Object(metadata),
            "spec": spec,
        });
        self.handle
            .block_on(self.api_for(key).patch(
                &key.name,
                &PatchParams::apply(manager).force(),
                &Patch::Apply(&object),
            ))
            .map(|_| ())
            .map_err(|error| err("apply", key, error))
    }

    fn adopt(
        &mut self,
        key: &ChildKey,
        desired_hash: &str,
        owner: &str,
    ) -> Result<(), FleetClusterError> {
        let patch = json!({
            "metadata": {
                "labels": { (FLEET_OWNER_LABEL): owner },
                "annotations": { (FLEET_HASH_ANNOTATION): desired_hash },
            }
        });
        self.handle
            .block_on(self.api_for(key).patch(
                &key.name,
                &PatchParams::default(),
                &Patch::Merge(patch),
            ))
            .map(|_| ())
            .map_err(|error| err("adopt", key, error))
    }

    fn delete(&mut self, key: &ChildKey) -> Result<(), FleetClusterError> {
        match self.handle.block_on(
            self.api_for(key)
                .delete(&key.name, &DeleteParams::default()),
        ) {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(error)) if error.code == 404 => Ok(()),
            Err(error) => Err(err("delete", key, error)),
        }
    }

    fn is_ready(&mut self, key: &ChildKey) -> Result<bool, FleetClusterError> {
        let found = self
            .handle
            .block_on(self.api_for(key).get_opt(&key.name))
            .map_err(|error| err("probe", key, error))?;
        let Some(object) = found else {
            return Ok(false);
        };
        let replicas = object.data["spec"]["replicas"].as_i64().unwrap_or(1);
        let ready_replicas = object.data["status"]["readyReplicas"].as_i64().unwrap_or(0);
        let observed_generation = object.data["status"]["observedGeneration"]
            .as_i64()
            .unwrap_or(0);
        let generation = object.metadata.generation.unwrap_or(0);
        Ok(ready_replicas >= replicas && observed_generation >= generation)
    }
}

#[cfg(test)]
mod tests;
