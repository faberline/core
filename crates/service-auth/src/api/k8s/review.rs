//! The seam between "ask the apiserver" and "decide what the answer means".
//!
//! Everything above this trait is pure: parsing, caching, and policy. Every
//! network round trip is behind [`ReviewBackend`]. That split is what lets the
//! decision logic be tested exhaustively — audience mismatch, malformed
//! responses, outages, revocation windows — without a cluster, and it is what
//! keeps this library free of any opinion about *which* Kubernetes client a
//! service links.
//!
//! The value types are deliberately generic. `ResourceAttributes` is the
//! Kubernetes shape, not any service's: a service maps its own operations onto
//! an API group, a resource, and a verb, and this library never learns what
//! those strings mean.

pub use crate::domain::k8s::{
    AccessReviewOutcome, ExtraFields, ResourceAttributes, ReviewBackend, ReviewError,
    TokenReviewOutcome,
};
