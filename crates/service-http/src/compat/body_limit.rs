//! Shared request-body byte cap for a service's data plane, with the
//! crate's `{error, message}` envelope on rejection (#2484).
//!
//! `HttpConfig::body_limit_bytes` (see [`crate::config`]) has always been a
//! documented, service-supplied knob, but nothing in this crate enforced
//! it: every known adopter either hand-rolled its own
//! `axum::extract::DefaultBodyLimit` literal that never actually read the
//! config field (tape, lumen's admin route, keep's own separate CLI flag),
//! or shipped its data plane with no cap at all. [`body_limit_layer`] is
//! the one place that enforcement now lives, so a service wires the config
//! field it already has instead of inventing another local constant.
//!
//! ## What it enforces
//!
//! - A `Content-Length` header over `max_bytes` is rejected immediately —
//!   before the request body is read at all.
//! - A request without (or under) `Content-Length` — chunked/streamed
//!   bodies — is still bounded: the body is wrapped in
//!   [`http_body_util::Limited`], so a streamed body that grows past
//!   `max_bytes` is caught mid-read rather than buffered without bound.
//!   Handlers that extract the body through axum's `Bytes`/`String`/
//!   `Json`/`Form` extractors get the resulting `413` for free — axum-core
//!   recognizes the wrapped body's length-limit error during extraction. A
//!   handler that reads the body through some other path is responsible for
//!   surfacing that error itself; this layer only guarantees the byte
//!   count is bounded, not that every possible body-consumption path
//!   renders a response.
//! - Every `413` this layer's response carries — whether short-circuited on
//!   `Content-Length` or produced downstream once the wrapped body errors
//!   mid-read — is rendered as the crate's [`crate::ErrorEnvelope`] JSON
//!   shape (`{"error": "payload_too_large", "message": ...}`), mirroring
//!   [`crate::admission::admission_middleware`]'s `429` envelope convention
//!   instead of axum's own plain-text rejection body.
//!
//! ## How a service wires it
//!
//! One layer at router composition, over the data plane only — probes stay
//! unbounded, matching this crate's documented probe behavior
//! ([`crate::probes::standard_probe_routes`]):
//!
//! ```ignore
//! let data_plane = my_routes()
//!     .layer(service_http::body_limit_layer(cfg.body_limit_bytes));
//! let app = service_http::standard_probe_routes(readiness, None, openapi)
//!     .merge(data_plane);
//! ```
//!
//! A service that also composes admission control or auth on the data
//! plane can stack this layer with those the same way `.layer(...)` stacks
//! any other tower layer; there is no required ordering relative to them
//! (the byte cap and the request's class/identity are independent checks).
//!
//! ## Recommended default
//!
//! 8 MiB (`8 * 1024 * 1024`) — the value [`crate::config`]'s own tests use
//! for `HttpConfig::body_limit_bytes`, and the literal every known adopter
//! independently converged on before this shared layer existed. A service
//! with materially larger legitimate payloads should size `max_bytes`
//! explicitly rather than inherit this default.

pub use crate::interfaces::{body_limit_layer, BodyLimitLayer, BodyLimitService};
