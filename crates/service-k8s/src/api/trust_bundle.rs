//! Public trust-bundle convergence API: publishing, adopting, rotating, and pruning
//! public CA bundles across declared client namespaces.

pub use crate::application::trust_bundle::{reconcile, TrustBundleReport};
pub use crate::domain::trust_bundle::{
    canonicalize_bundle_pem, client_trust_ready_condition, validate_public_certificates,
    ConfigMapStore, NamespaceOutcome, StoreError, StoreErrorKind, StoredConfigMap, TrustBundleSpec,
    TrustBundleValidationError, CA_BUNDLE_DATA_KEY, CLIENT_TRUST_READY_CONDITION,
    OWNER_IDENTITY_ANNOTATION, OWNER_IDENTITY_LABEL,
};
pub use crate::infrastructure::trust_bundle::InMemoryConfigMapStore;
#[cfg(any(feature = "certificate", feature = "controller"))]
pub use crate::infrastructure::trust_bundle::{
    classify_kube_error, KubernetesConfigMapStore, TRUST_BUNDLE_FIELD_MANAGER,
};
