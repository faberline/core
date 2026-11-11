//! Infrastructure stores for public trust bundle convergence.

#[cfg(any(feature = "certificate", feature = "controller"))]
pub mod kubernetes_store;
pub mod memory_store;

#[cfg(any(feature = "certificate", feature = "controller"))]
pub use kubernetes_store::{
    classify_kube_error, KubernetesConfigMapStore, TRUST_BUNDLE_FIELD_MANAGER,
};
pub use memory_store::InMemoryConfigMapStore;
