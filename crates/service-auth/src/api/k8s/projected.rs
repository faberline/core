//! Reading the projected ServiceAccount token a workload was given.
//!
//! The counterpart to the mounted volume: a client that calls an
//! audience-bound service opens this file immediately before each call. That
//! "immediately before" is the whole design.
//!
//! The kubelet rotates the projection in place — it writes the replacement
//! into a new directory and atomically re-points a symlink, at roughly 80% of
//! the token's lifetime. Nothing tells the process. A client that reads once
//! at startup therefore works perfectly for eight minutes and then fails
//! forever, which is the worst possible shape for the failure: it survives
//! every smoke test and breaks in the middle of the night. Re-reading is a
//! file open on tmpfs, so there is nothing to optimise away here.
//!
//! ## What is checked before the token leaves this module
//!
//! The client cannot verify the signature — it holds no key, and it is not the
//! audience. What it *can* do is refuse to send material that is already known
//! to be useless, and say why:
//!
//! - **Missing or unreadable** — the volume was never mounted, or the path
//!   disagrees with the manifest.
//! - **Wrong audience** — a default pod token got mounted instead of a
//!   projected one. This is the case that would otherwise reach the callee and
//!   come back as a bare `401`, sending whoever debugs it to look at RBAC.
//! - **Expired** — the kubelet's refresh did not happen, or the pod was
//!   suspended past the lifetime.
//!
//! Each is a distinct error naming the file and the audience, and none of them
//! carries the token. That is not decoration: a credential that reaches a log
//! line, a Kubernetes Event, or a CR status has left the pod, and the three
//! failures above are exactly the ones a maintainer is tempted to debug by
//! printing the value.

pub use crate::domain::k8s::ProjectedToken;
pub use crate::infrastructure::k8s::{ProjectedTokenError, ProjectedTokenFile};
