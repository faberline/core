//! The port that holds access grant objects (Role and RoleBinding) in a cluster.

use std::collections::BTreeMap;

use futures::future::BoxFuture;
use serde_json::Value;

use crate::domain::trust_bundle::StoreError;

/// The kinds of object that an access grant renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessObjectKind {
    Role,
    RoleBinding,
}

/// Reads, applies and deletes the Role and RoleBinding of an access grant.
/// Copies the shape of `ConfigMapStore`.
pub trait AccessStore: Send + Sync {
    /// The labels of the stored object, or `None` when it does not exist.
    fn read<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<BTreeMap<String, String>>, StoreError>>;

    /// Create or replace the object with `object`.
    fn apply<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
        object: Value,
    ) -> BoxFuture<'a, Result<(), StoreError>>;

    /// Delete the object; succeeds when it is absent.
    fn delete<'a>(
        &'a self,
        kind: AccessObjectKind,
        namespace: &'a str,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), StoreError>>;
}
