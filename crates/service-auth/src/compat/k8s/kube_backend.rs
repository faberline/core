//! The only part of delegated auth that talks to a network.
//!
//! Everything interesting about this file is defensive. A `TokenReview`
//! response is a document the apiserver returns with HTTP 201, and a
//! `SubjectAccessReview` response is the same — a 2xx status says the *request*
//! was accepted, not that it was answered. So each response is translated
//! field by field, and anything that is not a complete answer becomes a
//! [`ReviewError`] rather than a default-valued outcome. A `TokenReviewStatus`
//! deserialized into `Default::default()` would read as `authenticated: false`,
//! which is safe, but a `SubjectAccessReviewStatus` default reads as
//! `allowed: false` *and* `denied: false` — which is "no opinion", not "no".
//! Distinguishing those is the reason this translation is written out instead
//! of derived.

pub use crate::infrastructure::k8s::kube_backend::KubeReviewBackend;
