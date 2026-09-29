//! The five standard probe/admin endpoints every k8s-native service ships
//! (CONTRIBUTING.md "standard endpoints"): `/healthz`, `/readyz`, `/metrics`,
//! `/openapi.json`, `/docs`.
//!
//! These routes carry **no auth and no body limit** — k8s liveness/readiness
//! probes and Prometheus scrape must reach them token-free even when the data
//! plane requires auth. A service merges its own (auth'd, body-limited) data
//! plane onto the router returned here. This is the exact shape lumen
//! (`api::router`) and keep (`http::routes::router`) hand-roll today.

pub use crate::interfaces::{
    lifecycle_probe_routes, lifecycle_probe_routes_canonical_json, standard_probe_routes,
    standard_probe_routes_canonical_json,
};
