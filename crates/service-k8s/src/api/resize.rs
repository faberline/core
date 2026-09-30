//! PVC resize support: parse Kubernetes storage quantities, decide whether a
//! PVC needs growing, and patch `spec.resources.requests.storage` on PVCs
//! whose bound `StorageClass` allows expansion.
//!
//! StatefulSet `volumeClaimTemplates` are immutable after creation, so
//! bumping a CR's declared storage size and letting the operator reconcile
//! does **not** resize anything — the rendered StatefulSet's `apply` is a
//! silent no-op for that field. This module is the detect-and-patch tool for
//! the gap: pure quantity comparison ([`parse_storage_bytes`], [`decide`])
//! plus an impure driver ([`resize_instance`]) that lists a namespace's live
//! PVCs matching a label selector + name filter, compares each against a
//! caller-supplied desired size, and patches `spec.resources.requests.storage`
//! directly on PVCs whose bound `StorageClass` allows expansion. PVC shrink
//! is never attempted — Kubernetes does not support it.
//!
//! Lifted from lumen's `service_k8s::resize` (#970): this module knows nothing
//! about any particular CRD. The caller resolves the desired size (typically
//! read from its own CR's spec) and supplies it as an accessor, plus a label
//! selector and a PVC-name filter to scope the listing to one instance.

pub use crate::infrastructure::resize::{
    decide, parse_storage_bytes, resize_instance, PvcResizeOutcome, ResizeAction,
};
