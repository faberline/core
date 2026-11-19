//! The offline `k8s ... render` rules the service CLIs share: the names of the
//! rendered files, the `--out` rules and the `next:` marker. Each service
//! passes its own names, rule and marker.

pub use crate::domain::k8s_render::{
    next_line, render_instance, Artifact, DefaultFileNames, InstanceInputs, InstanceSpec,
    NextMarker, OutRule, Profile,
};
pub use crate::interfaces::k8s_render::{emit, out_target};
