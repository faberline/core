//! Handing a credential to a program without handing it the credential.
//!
//! A CLI that wraps another command — `foo connect -- curl ...`, `foo connect
//! -- pytest` — has to get an authenticated connection to the child somehow.
//! The obvious way is an environment variable, and it is the wrong way: the
//! child's environment is inherited by every descendant it spawns, readable
//! from `/proc/<pid>/environ` by anything running as the same user, and
//! captured verbatim by most crash reporters and process supervisors. A token
//! put there has been handed to a much larger set of programs than the one
//! that was wrapped.
//!
//! So the child is given a URL instead. The token stays in the parent's
//! address space, and this proxy attaches it to each request as it passes:
//!
//! ```text
//!   child process  --http://127.0.0.1:<ephemeral>-->  LoopbackProxy
//!   (holds a URL and nothing else)                          |
//!                                        Authorization: Bearer <token from
//!                                        TokenSource, refreshed as needed>
//!                                                            v
//!                                                        upstream
//! ```
//!
//! Two properties make that worth the machinery rather than security
//! decoration:
//!
//! - **The listener is loopback-only and ephemeral.** It binds `127.0.0.1:0`,
//!   so nothing off the host can reach it and no port is predictable between
//!   runs. It is still reachable by any process on the host running as the
//!   same user — which is the same trust boundary the child itself sits in,
//!   and strictly smaller than the environment-variable boundary, which also
//!   includes anything that inherits or reads that environment later.
//! - **A refresh failure is a refusal.** When the token cannot be renewed —
//!   the grant was revoked, the kubeconfig expired — the proxy answers `503`
//!   and reports the failure on [`LoopbackProxy::next_fatal`] so the caller
//!   can tear the whole thing down. Continuing to forward the token already in
//!   hand until it expires would convert a revocation into a delay.
//!
//! Any inbound `Authorization` header is replaced, not merged: the proxy is
//! the only thing that decides what identity these requests carry, and a child
//! that sets its own must not be able to talk past it.

pub use crate::infrastructure::k8s::verifying_client;
pub use crate::interfaces::loopback_proxy::LoopbackProxy;
