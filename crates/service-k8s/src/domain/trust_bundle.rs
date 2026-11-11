//! Domain models and abstractions for public trust bundle convergence across client namespaces.

pub mod condition;
pub mod spec;
pub mod store;
pub mod validation;

pub use condition::{client_trust_ready_condition, NamespaceOutcome, CLIENT_TRUST_READY_CONDITION};
pub use spec::{
    TrustBundleSpec, CA_BUNDLE_DATA_KEY, OWNER_IDENTITY_ANNOTATION, OWNER_IDENTITY_LABEL,
};
pub use store::{ConfigMapStore, StoreError, StoreErrorKind, StoredConfigMap};
pub use validation::{
    canonicalize_bundle_pem, validate_public_certificates, TrustBundleValidationError,
};
