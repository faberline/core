//! A secret store held in memory, for driving the lifecycle without a cluster.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::domain::certificate::secret_store::{SecretStore, StoreError, StoredSecret};

/// An in-memory [`SecretStore`], so the lifecycle can be driven without a
/// cluster. Shipped rather than test-gated for the same reason as
/// [`super::ephemeral::EphemeralIssuer`]: R8's no-cloud build has to be able to
/// exercise something.
pub struct MemoryStore {
    secrets: std::sync::Mutex<BTreeMap<String, StoredSecret>>,
    applies: std::sync::atomic::AtomicU64,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            secrets: std::sync::Mutex::new(BTreeMap::new()),
            applies: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn get(&self, namespace: &str, name: &str) -> Option<StoredSecret> {
        self.secrets
            .lock()
            .expect("memory store")
            .get(&format!("{namespace}/{name}"))
            .cloned()
    }

    pub fn apply_count(&self) -> u64 {
        self.applies.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl SecretStore for MemoryStore {
    fn read<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<Option<StoredSecret>, StoreError>> {
        Box::pin(futures::future::ready(Ok(self.get(namespace, name))))
    }

    fn apply<'a>(
        &'a self,
        object: Value,
    ) -> futures::future::BoxFuture<'a, Result<(), StoreError>> {
        let namespace = object["metadata"]["namespace"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let name = object["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let mut secrets = self.secrets.lock().expect("memory store");
        let mut changed = false;
        let entry = match secrets.entry(format!("{namespace}/{name}")) {
            std::collections::btree_map::Entry::Vacant(v) => {
                changed = true;
                v.insert(StoredSecret::default())
            }
            std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
        };
        // Merge semantics, matching a server-side apply of the fields present:
        // a trust-bundle-only apply must leave the leaf keys where they are.
        if let Some(data) = object["stringData"].as_object() {
            for (key, value) in data {
                if let Some(text) = value.as_str() {
                    let bytes = text.as_bytes().to_vec();
                    if entry.data.get(key) != Some(&bytes) {
                        entry.data.insert(key.clone(), bytes);
                        changed = true;
                    }
                }
            }
        }
        if let Some(annotations) = object["metadata"]["annotations"].as_object() {
            for (key, value) in annotations {
                if let Some(text) = value.as_str() {
                    if entry.annotations.get(key) != Some(&text.to_string()) {
                        entry.annotations.insert(key.clone(), text.to_string());
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.applies
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Box::pin(futures::future::ready(Ok(())))
    }
}
