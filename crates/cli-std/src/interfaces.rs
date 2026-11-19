//! Presentation helpers for service CLIs: deployment-artifact rendering.

pub(crate) mod artifact;
#[cfg(feature = "k8s")]
pub(crate) mod k8s_render;
