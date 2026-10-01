//! W3C `Server-Timing` on every response — per-request latency attribution
//! for integrators who have no access to Prometheus metrics or logs.
//!
//! [`server_timing_middleware`] adds one `Server-Timing` response header to
//! every request it wraps:
//!
//! - **Baseline, always present:** `app;dur=<ms>` — the wall-clock time from
//!   this middleware's `next.run` entry to the response leaving it. That is
//!   the same request/response boundary [`crate::transport::trace_layer`]
//!   spans; compose both onto the **same, outermost** layer position (see
//!   the crate root's "What a service wires" example) so `app;dur=` and the
//!   trace span's recorded latency describe the same measurement window
//!   instead of two independently-drifting notions of "how long did this
//!   take". tower-http's `OnResponse::on_response` hook only gets an
//!   immutable `&Response` alongside the latency it measured — there is no
//!   hook to write a header from a value it already computed, so this
//!   middleware cannot literally reuse `TraceLayer`'s internal timer without
//!   forking tower-http; composing at the same outer boundary is the closest
//!   a header-writing layer can get to "don't re-time" without doing that.
//! - **Phase entries, opt-in per response:** handlers push named durations
//!   onto the [`ServerTimingExt`] extension this middleware inserts into
//!   every request (`ext.push("search", elapsed)`), and they render after
//!   `app;dur=` — but only on responses that carry
//!   [`ServerTimingDisclosure::Full`] (see below).
//!
//! ## Disclosure posture — decided once, here
//!
//! The issue that motivated this module asks for total-only breakdown on
//! unauthenticated requests and a full phase breakdown once a request
//! carries a successful auth context. `service-http` cannot make that
//! distinction today:
//!
//! - This crate does not depend on `service-auth`.
//! - `service-auth`'s `auth_middleware<V>` inserts the concrete,
//!   per-service `V::Principal` (e.g. lumen's `AuthContext`) into
//!   **request** extensions on success. There is no crate-neutral "this
//!   request authenticated" marker type — every adopter's principal type is
//!   different, and this crate would have to name one of them to look for
//!   it.
//! - Nothing publishes a **response**-side authentication signal either
//!   (the only place a middleware positioned outside the whole stack, the
//!   way `server_timing_middleware` is meant to be, can observe anything
//!   after the handler has run).
//!
//! Given that, the posture is decided conservatively, once: **every**
//! response defaults to [`ServerTimingDisclosure::TotalOnly`] — `app;dur=`
//! only, no phase entries — regardless of the request's auth state. This is
//! the safe default the originating issue calls out explicitly for the case
//! where "auth state isn't visible at this layer".
//!
//! The hook for later: any layer or handler nested inside
//! `server_timing_middleware` (so, anything that runs during `next.run` —
//! including a service's own auth middleware or the handler itself) may
//! flip one response to full disclosure by inserting
//! `ServerTimingDisclosure::Full` into that **response's** extensions
//! before it returns:
//!
//! ```ignore
//! use axum::response::IntoResponse;
//! use service_http::ServerTimingDisclosure;
//!
//! async fn handler() -> axum::response::Response {
//!     let mut response = "ok".into_response();
//!     // e.g. once request-side auth state is confirmed successful:
//!     response.extensions_mut().insert(ServerTimingDisclosure::Full);
//!     response
//! }
//! ```
//!
//! `server_timing_middleware` never inspects a principal type or
//! credentials itself — it only ever looks for this one marker on the
//! response it gets back. Wiring that marker from a real auth success (in
//! `service-auth` or a service's own auth layer) is deliberately left as
//! follow-up work, not part of this change.

pub use crate::interfaces::{server_timing_middleware, ServerTimingDisclosure, ServerTimingExt};
