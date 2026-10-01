//! The certificate lifecycle's adapters: the Kubernetes Secret store and
//! layout, the in-memory store, the in-process issuer, and the CA Service client.

#[cfg(feature = "gcp-cas-client")]
pub(crate) mod cas;
pub(crate) mod ephemeral;
pub(crate) mod kubernetes_store;
pub(crate) mod memory_store;
pub(crate) mod secret_layout;
