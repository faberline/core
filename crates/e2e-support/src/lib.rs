//! Shared end-to-end test harness: pure checks for identity, pool placement,
//! inspect helpers and evidence lines (`domain`), and the `kubectl`, wait and
//! HTTP helpers that run them (`infrastructure`).

mod domain;
mod infrastructure;

pub use domain::{
    env_count, evidence_line, is_dns_label, is_lower_hex, mounts_named, named,
    normalize_runtime_image_id, Identity, PoolPlacement, EVIDENCE_PREFIX,
};
pub use infrastructure::{
    evidence, namespace, required, say, wait_for, wait_pods_exist, wait_ready, Api, DumpOnFailure,
    Kube, Step,
};
