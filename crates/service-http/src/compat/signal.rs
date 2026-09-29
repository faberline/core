//! Service-shell re-exports for protocol-neutral graceful shutdown.
//!
//! The `shutdown_with_drain` and `wait_shutdown_signal` exports are legacy
//! adapters. Production code should use [`LifecycleShutdownTrigger`] with the
//! caller-owned lifecycle and `run_signal_bridge` (or `shutdown_on_signal`).
//!
//! The drain dance every k8s-native service in the ecosystem repeats: on
//! SIGINT/SIGTERM, flip readiness to draining (so `/readyz` → 503 and k8s stops
//! routing), hold a grace window, then let the listener close. Factored out of
//! lumen's / keep's `shutdown_signal`. Ownership lives in `server-lifecycle`.

pub use crate::interfaces::{
    run_signal_bridge, shutdown_on_signal, shutdown_with_drain, wait_shutdown_signal,
    LifecycleShutdownTrigger,
};
