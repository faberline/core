//! Shared end-to-end test harness: pure checks for identity, pool placement,
//! inspect helpers and evidence lines. No I/O; the I/O layer comes later.

mod domain;

pub use domain::{
    env_count, evidence_line, is_dns_label, is_lower_hex, mounts_named, named,
    normalize_runtime_image_id, Identity, PoolPlacement, EVIDENCE_PREFIX,
};
