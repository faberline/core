//! The offline `k8s ... render` rules the service CLIs share: the names of the
//! rendered files, the `--out` rules and the `next:` marker. Each service
//! passes its own names, rule and marker. It also holds the CRD and operator
//! render rules.

pub use crate::domain::k8s_render::{
    next_line, render_crd, render_instance, render_operator, validate_image, validate_namespace,
    validate_token, Artifact, DefaultFileNames, InstanceInputs, InstanceSpec, NamespaceRule,
    NextMarker, OperatorParts, OutRule, Profile, RenderError,
};
pub use crate::interfaces::k8s_render::{emit, out_target};
