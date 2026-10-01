//! Minimal Lease-based leader election (coordination.k8s.io/v1).
//!
//! kube-rs 0.98 ships no built-in elector, so this is a small hand-rolled one:
//! every operator replica runs the watch + reconcile loop, but only the replica
//! that currently holds the `<manager>` Lease actually applies changes (the
//! reconcile loop gates on [`Election::is_leader`]). A background task
//! acquires/renews the Lease; if the holder's renewal lapses past the lease
//! duration, another replica takes over. This makes `replicas > 1` safe (no two
//! reconcilers fighting) without an external dependency.
//!
//! Lifted from lumen's operator; the Lease name is now a parameter (the
//! service's `MANAGER`) so two different operators never share one Lease.

pub use crate::domain::leadership::Election;
pub use crate::infrastructure::lease::spawn;
