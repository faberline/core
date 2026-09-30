//! Short-lived certificates, reconciled.
//!
//! Serving TLS and Raft peer mTLS both need a leaf that expires in hours, gets
//! replaced before it does, and is signed by a CA the rest of the fleet already
//! trusts. That is a control loop, and this module is it: the shared, service-
//! neutral half. A service supplies a [`profile::CertificateProfile`] naming
//! what it wants; everything about *when* and *in what order* lives here.
//!
//! ```text
//!   profile   what identity, for how long, renewed how early
//!   state     given what the cluster shows and what time it is, do what
//!   issuer    the boundary: a CSR goes out, a leaf comes back
//!   projection where material lands, and what is read back from where
//!   status    what any of this is allowed to say about itself
//!   reconcile the seam -- authorize, read, decide, write
//! ```
//!
//! The layering has one rule, and it is worth stating because it is what makes
//! the whole thing testable: **nothing above `issuer` knows which CA is in
//! use.** [`ephemeral::EphemeralIssuer`] signs in-process and
//! [`cas::CasIssuer`] calls GCP CA Service; the state machine cannot tell them
//! apart, so every property about renewal timing, rotation ordering, and expiry
//! is checked in a unit test rather than in a cloud gate somebody remembers to
//! run.
//!
//! ### What this deliberately does not do
//!
//! It does not apply Terraform, restart Pods, or edit a Deployment. Renewal is
//! a Secret write and nothing else — the trust foundation is provisioned once
//! (#3109) and the runtime picks up new material without a restart (#3112). A
//! renewal path that shelled out to infrastructure would make certificate
//! expiry an operational event, which is the thing short-lived certificates
//! exist to stop being.

pub mod digest {
    pub use crate::domain::certificate::digest::hex_sha256;
}
pub mod ephemeral {
    pub use crate::infrastructure::certificate::ephemeral::{instant, EphemeralIssuer};
}
pub mod issuer {
    pub use crate::domain::certificate::issuer::{
        IssuanceRequest, IssuedMaterial, Issuer, IssuerError, IssuerId, KeyAndCsrGenerator,
        PrivateKey,
    };
    pub use crate::infrastructure::certificate::csr::RcgenCsrGenerator;
}
pub mod kubernetes_store {
    pub use crate::infrastructure::certificate::kubernetes_store::{
        classify_kube_error, prepare_ssa_patch, KubernetesSecretStore, KubernetesStoreError,
        FIELD_MANAGER, LIFECYCLE_ANNOTATION_KEYS, LIFECYCLE_DATA_KEYS, LIFECYCLE_LABEL_KEYS,
        RBAC_VERBS, REQUIRED_RBAC_VERBS,
    };
}
pub mod profile {
    pub use crate::domain::certificate::profile::{
        CertificateIdentity, CertificateProfile, ExtendedUsage, InstanceScope, ProfileError,
        Purpose, MAX_LIFETIME_SECS, MIN_LIFETIME_SECS, MIN_RENEW_BEFORE_SECS,
    };
}
pub mod projection {
    pub use crate::domain::certificate::projection::{
        Owner, ProjectedState, TrustBundle, CERT_KEY, IDENTITY_DIGEST_ANNOTATION,
        LEAF_ISSUER_ANNOTATION, PRIVATE_KEY_KEY, TRUST_BUNDLE_ANNOTATION, TRUST_BUNDLE_KEY,
    };
    pub use crate::domain::certificate::secret_layout::{
        material_secret, read_state, trust_bundle_secret, LeafFacts, LeafParser,
    };
    pub use crate::infrastructure::certificate::leaf_parser::{parse_leaf, X509LeafParser};
}
pub mod reconcile {
    pub use crate::application::certificate::reconcile::{
        Outcome, ReconcileError, Reconciler, RuntimeReport, PROJECTED_KEYS,
    };
    pub use crate::domain::certificate::secret_store::{
        SecretStore, StoreError, StoreErrorKind, StoredSecret,
    };
    pub use crate::infrastructure::certificate::memory_store::MemoryStore;
}
pub mod state {
    pub use crate::domain::certificate::state::{
        next_action, renew_at, retry_after, Action, Desired, IssueReason, Observed, ObservedLeaf,
    };
}
pub mod status {
    pub use crate::domain::certificate::status::{
        redact, CertificateFacts, READY_CONDITION, ROTATING_CONDITION,
    };
}

#[cfg(feature = "gcp-cas-client")]
pub mod cas {
    pub use crate::infrastructure::certificate::cas::{
        AccessTokenSource, CaPool, CasIssuer, GkeMetadataTokenSource, WorkloadIdentityTokenSource,
    };
}

pub use ephemeral::EphemeralIssuer;
pub use issuer::{
    IssuanceRequest, IssuedMaterial, Issuer, IssuerError, IssuerId, KeyAndCsrGenerator, PrivateKey,
    RcgenCsrGenerator,
};
pub use kubernetes_store::{
    classify_kube_error, prepare_ssa_patch, KubernetesSecretStore, KubernetesStoreError,
    FIELD_MANAGER, RBAC_VERBS, REQUIRED_RBAC_VERBS,
};
pub use profile::{
    CertificateIdentity, CertificateProfile, ExtendedUsage, InstanceScope, ProfileError, Purpose,
};
pub use projection::{LeafParser, Owner, ProjectedState, TrustBundle, X509LeafParser};
pub use reconcile::{
    MemoryStore, Outcome, ReconcileError, Reconciler, RuntimeReport, SecretStore, StoreError,
    StoreErrorKind, StoredSecret,
};
pub use state::{
    next_action, renew_at, retry_after, Action, Desired, IssueReason, Observed, ObservedLeaf,
};
pub use status::{redact, CertificateFacts, READY_CONDITION, ROTATING_CONDITION};

#[cfg(feature = "gcp-cas-client")]
pub use cas::{
    AccessTokenSource, CaPool, CasIssuer, GkeMetadataTokenSource, WorkloadIdentityTokenSource,
};
