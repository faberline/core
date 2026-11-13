//! In-memory implementation of [`AccessStore`] for tests and cluster-free execution.

use std::collections::BTreeMap;
use std::sync::Mutex;

use futures::future::{ready, BoxFuture};
use serde_json::Value;

use crate::domain::access::{AccessObjectKind, AccessStore};
use crate::domain::trust_bundle::StoreError;

type Key = (AccessObjectKind, String, String);

/// An in-memory [`AccessStore`]. Every trait call is recorded in order.
pub struct InMemoryAccessStore {
    objects: Mutex<BTreeMap<Key, Value>>,
    calls: Mutex<Vec<String>>,
    failures: Mutex<BTreeMap<String, StoreError>>,
}

impl Default for InMemoryAccessStore {
    fn default() -> Self {
        Self::new()
    }
}

fn key(kind: AccessObjectKind, namespace: &str, name: &str) -> Key {
    (kind, namespace.to_string(), name.to_string())
}

impl InMemoryAccessStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            objects: Mutex::new(BTreeMap::new()),
            calls: Mutex::new(Vec::new()),
            failures: Mutex::new(BTreeMap::new()),
        }
    }

    /// Make every `read`, `apply` and `delete` in `namespace` fail with `error`.
    pub fn fail_namespace(&self, namespace: &str, error: StoreError) {
        self.failures
            .lock()
            .expect("access store failures lock")
            .insert(namespace.to_string(), error);
    }

    /// Stop failing operations in `namespace`.
    pub fn clear_namespace_failure(&self, namespace: &str) {
        self.failures
            .lock()
            .expect("access store failures lock")
            .remove(namespace);
    }

    /// The stored object, or `None`. Not recorded as a call.
    pub fn get(&self, kind: AccessObjectKind, namespace: &str, name: &str) -> Option<Value> {
        self.objects
            .lock()
            .expect("access store objects lock")
            .get(&key(kind, namespace, name))
            .cloned()
    }

    /// Store an object directly. Not recorded as a call.
    pub fn insert(&self, kind: AccessObjectKind, namespace: &str, name: &str, object: Value) {
        self.objects
            .lock()
            .expect("access store objects lock")
            .insert(key(kind, namespace, name), object);
    }

    /// One entry per trait call, in order, such as `"read Role app-ns/name"`.
    /// An `apply` entry ends with ` kind=<the object's kind>`.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("access store calls lock").clone()
    }

    fn record(&self, call: String) {
        self.calls
            .lock()
            .expect("access store calls lock")
            .push(call);
    }

    fn failure(&self, namespace: &str) -> Option<StoreError> {
        self.failures
            .lock()
            .expect("access store failures lock")
            .get(namespace)
            .cloned()
    }
}

fn labels_of(object: &Value) -> BTreeMap<String, String> {
    object["metadata"]["labels"]
        .as_object()
        .map(|labels| {
            labels
                .iter()
                .filter_map(|(name, value)| value.as_str().map(|v| (name.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

impl AccessStore for InMemoryAccessStore {
    fn read<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<BTreeMap<String, String>>, StoreError>> {
        self.record(format!("read {kind:?} {namespace}/{name}"));
        let result = match self.failure(namespace) {
            Some(error) => Err(error),
            None => Ok(self
                .get(kind, namespace, name)
                .map(|object| labels_of(&object))),
        };
        Box::pin(ready(result))
    }

    fn apply<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        let object_kind = object["kind"].as_str().unwrap_or_default().to_string();
        self.record(format!(
            "apply {kind:?} {namespace}/{name} kind={object_kind}"
        ));
        let result = match self.failure(namespace) {
            Some(error) => Err(error),
            None => {
                self.insert(kind, namespace, name, object);
                Ok(())
            }
        };
        Box::pin(ready(result))
    }

    fn delete<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        self.record(format!("delete {kind:?} {namespace}/{name}"));
        let result = match self.failure(namespace) {
            Some(error) => Err(error),
            None => {
                self.objects
                    .lock()
                    .expect("access store objects lock")
                    .remove(&key(kind, namespace, name));
                Ok(())
            }
        };
        Box::pin(ready(result))
    }
}
