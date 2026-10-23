//! In-memory implementation of [`ConfigMapStore`] for testing and cluster-free execution.

use futures::future::BoxFuture;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::domain::trust_bundle::{ConfigMapStore, StoreError, StoredConfigMap};

/// An in-memory [`ConfigMapStore`].
pub struct InMemoryConfigMapStore {
    maps: Mutex<BTreeMap<String, StoredConfigMap>>,
    reads: AtomicU64,
    applies: AtomicU64,
    deletes: AtomicU64,
}

impl Default for InMemoryConfigMapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryConfigMapStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            maps: Mutex::new(BTreeMap::new()),
            reads: AtomicU64::new(0),
            applies: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
        }
    }

    /// Read an existing ConfigMap directly from memory without counting as a store call.
    pub fn get(&self, namespace: &str, name: &str) -> Option<StoredConfigMap> {
        self.maps
            .lock()
            .expect("memory store lock")
            .get(&format!("{namespace}/{name}"))
            .cloned()
    }

    /// Seed a ConfigMap directly into memory.
    pub fn insert(&self, namespace: &str, name: &str, map: StoredConfigMap) {
        self.maps
            .lock()
            .expect("memory store lock")
            .insert(format!("{namespace}/{name}"), map);
    }

    /// Number of `read` operations called through the [`ConfigMapStore`] interface.
    pub fn read_count(&self) -> u64 {
        self.reads.load(Ordering::SeqCst)
    }

    /// Number of `apply` operations called through the [`ConfigMapStore`] interface.
    pub fn apply_count(&self) -> u64 {
        self.applies.load(Ordering::SeqCst)
    }

    /// Number of `delete` operations called through the [`ConfigMapStore`] interface.
    pub fn delete_count(&self) -> u64 {
        self.deletes.load(Ordering::SeqCst)
    }
}

impl ConfigMapStore for InMemoryConfigMapStore {
    fn read<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<StoredConfigMap>, StoreError>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let res = self.get(namespace, name);
        Box::pin(futures::future::ready(Ok(res)))
    }

    fn apply<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        self.applies.fetch_add(1, Ordering::SeqCst);
        let mut data_map = BTreeMap::new();
        if let Some(data) = object.get("data").and_then(|d| d.as_object()) {
            for (k, v) in data {
                if let Some(text) = v.as_str() {
                    data_map.insert(k.clone(), text.to_string());
                }
            }
        }

        let mut labels_map = BTreeMap::new();
        if let Some(labels) = object
            .get("metadata")
            .and_then(|m| m.get("labels"))
            .and_then(|l| l.as_object())
        {
            for (k, v) in labels {
                if let Some(text) = v.as_str() {
                    labels_map.insert(k.clone(), text.to_string());
                }
            }
        }

        let mut annotations_map = BTreeMap::new();
        if let Some(annotations) = object
            .get("metadata")
            .and_then(|m| m.get("annotations"))
            .and_then(|a| a.as_object())
        {
            for (k, v) in annotations {
                if let Some(text) = v.as_str() {
                    annotations_map.insert(k.clone(), text.to_string());
                }
            }
        }

        let stored = StoredConfigMap {
            data: data_map,
            labels: labels_map,
            annotations: annotations_map,
        };

        self.insert(namespace, name, stored);
        Box::pin(futures::future::ready(Ok(())))
    }

    fn delete<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.maps
            .lock()
            .expect("memory store lock")
            .remove(&format!("{namespace}/{name}"));
        Box::pin(futures::future::ready(Ok(())))
    }
}
