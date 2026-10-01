//! The certificate lifecycle's adapters: the Kubernetes Secret store, the
//! in-memory store, the key and CSR generator, the leaf parser, the in-process
//! issuer, and the CA Service client.

#[cfg(feature = "gcp-cas-client")]
pub(crate) mod cas;
pub(crate) mod csr;
pub(crate) mod ephemeral;
pub(crate) mod kubernetes_store;
pub(crate) mod leaf_parser;
pub(crate) mod memory_store;
