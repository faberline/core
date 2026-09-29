//! How long a delegated answer may be reused, and what happens when the
//! apiserver stops answering.
//!
//! Every cached decision is a small window during which a revocation has not
//! taken effect yet. That window is the entire security cost of caching, so it
//! is stated as a number rather than left to emerge: an allow survives
//! [`CachePolicy::allow_ttl`], a deny survives the much shorter
//! [`CachePolicy::deny_ttl`] (a deny that outlives a freshly granted RoleBinding
//! is an availability bug, not a safety one), and when the apiserver is
//! unreachable an already-expired entry may be served for at most
//! [`CachePolicy::stale_window`] beyond that.
//!
//! The stale window is deliberately not reachable on the happy path. [`get`]
//! never returns an expired entry; only [`get_stale`] does, and the caller may
//! only reach for it after a review actually failed. That is what keeps the
//! worst case bounded at `ttl + stale_window` instead of "until the apiserver
//! comes back".
//!
//! [`get`]: TtlCache::get
//! [`get_stale`]: TtlCache::get_stale

pub use crate::application::k8s::SystemClock;
pub use crate::domain::k8s::{CacheOutcome, CachePolicy, Clock, ManualClock, TtlCache};
