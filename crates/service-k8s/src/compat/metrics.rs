//! The operator's view of itself.
//!
//! Every service in the kit runs the same controller, so until now every
//! service was blind in the same place: [`crate::controller::run`] started a
//! reconcile loop and nothing else, exposing no port, counting no reconciles,
//! and losing every error into a bare requeue. A control plane that cannot say
//! whether it is converging is indistinguishable, from outside the process,
//! from one that is idle — which is the exact failure this module exists to
//! make impossible (#2620).
//!
//! The metric set is deliberately small and derived from one question: *is this
//! operator doing its job?* That needs a rate of work (`_reconcile_total`), a
//! rate of failure (`_reconcile_errors_total`), a latency distribution
//! (`_reconcile_duration_seconds`), and which replica is actually allowed to
//! act (`_leader`). Anything beyond that is a service's own business and
//! belongs on the instance endpoint, not here.
//!
//! Names are prefixed from the service's `MANAGER` (`lumen-operator` →
//! `lumen_operator_`), so the six services sharing this controller land in one
//! Prometheus without colliding, and a query written against one reads the same
//! for all of them.

pub use crate::interfaces::metrics::{
    metrics_addr, serve, ControllerMetrics, DEFAULT_METRICS_ADDR, METRICS_ADDR_ENV,
};
