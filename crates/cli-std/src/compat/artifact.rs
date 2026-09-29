//! Shared rendering helpers for service CLI deployment artifacts.
//!
//! K8s-native service CLIs all render the same classes of byte artifacts:
//! checked-in Dockerfiles, operator manifests, and CRD/instance YAML. The
//! service owns its domain-specific body; this module owns the presentation
//! hygiene that must remain uniform across those CLIs.

pub use crate::interfaces::artifact::{
    ensure_trailing_newline, release_tag, replace_kubernetes_namespace,
    strip_source_ownership_markers, write_or_print,
};
