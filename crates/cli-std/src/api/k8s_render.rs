//! The offline `k8s ... render` rules the service CLIs share: the names of the
//! rendered files, the `--out` rules and the `next:` marker. Each service
//! passes its own names, rule and marker. It also holds the CRD and operator
//! render rules. It also holds the collector render, the operator default,
//! no-feature message and log format rules.

pub use crate::domain::k8s_render::{
    next_line, no_operator_feature_message, operator_default_is_run, parse_log_format,
    render_collector, render_crd, render_instance, render_operator, validate_image,
    validate_namespace, validate_token, Artifact, DefaultFileNames, InstanceInputs, InstanceSpec,
    LogFormat, NamespaceRule, NextMarker, OperatorDefault, OperatorParts, OutRule, Profile,
    RenderError,
};
pub use crate::interfaces::k8s_render::{emit, out_target};
