//! The offline `k8s ... render` rules: the names of the rendered files, the
//! `next:` marker policy and the `--out` rule. Pure values only; the file
//! system and stdout live in `interfaces::k8s_render`.

use std::path::Path;

/// One rendered artifact of a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artifact {
    /// The custom resource definition.
    Crd,
    /// The operator deployment.
    Operator,
    /// The service instance.
    Instance,
    /// The collector (the sift names only).
    Collector,
}

/// The default file names of a service's rendered artifacts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultFileNames {
    /// The CRD file name.
    pub crd: String,
    /// The operator file name.
    pub operator: String,
    /// The instance file name.
    pub instance: String,
    /// The collector file name, when the service has a collector.
    pub collector: Option<String>,
}

impl DefaultFileNames {
    /// `crd.yaml`, `operator.yaml`, `<service>.yaml`, no collector.
    pub fn for_service(service: &str) -> Self {
        Self {
            crd: "crd.yaml".to_string(),
            operator: "operator.yaml".to_string(),
            instance: format!("{service}.yaml"),
            collector: None,
        }
    }

    /// `<service>-crd.yaml`, `<service>-operator.yaml`, `<service>.yaml`,
    /// `Some("<service>-collector.yaml")` (the sift names).
    pub fn prefixed(service: &str) -> Self {
        Self {
            crd: format!("{service}-crd.yaml"),
            operator: format!("{service}-operator.yaml"),
            instance: format!("{service}.yaml"),
            collector: Some(format!("{service}-collector.yaml")),
        }
    }

    /// The default file name of `artifact`; `None` for `Artifact::Collector`
    /// when `collector` is `None`.
    pub fn file_for(&self, artifact: Artifact) -> Option<&str> {
        match artifact {
            Artifact::Crd => Some(self.crd.as_str()),
            Artifact::Operator => Some(self.operator.as_str()),
            Artifact::Instance => Some(self.instance.as_str()),
            Artifact::Collector => self.collector.as_deref(),
        }
    }
}

/// When a render prints its `next:` line after the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextMarker {
    /// Print no `next:` line.
    None,
    /// Print `next: done` only after the output was written to a file.
    DoneAfterFileWrite,
    /// Print `next: done` always.
    DoneAlways,
    /// Print the `kubectl apply -f` command for the output.
    ApplyCommand,
}

/// How `--out` names the file that a render writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutRule {
    /// A path with an extension is the file; any other path is a directory.
    ByExtension,
    /// An existing directory is a directory; any other path is the file.
    ExistingDirectory,
}

/// The `next:` line a render prints after its output, or `None`.
pub fn next_line(marker: NextMarker, written: Option<&Path>) -> Option<String> {
    match (marker, written) {
        (NextMarker::None, _) => None,
        (NextMarker::DoneAfterFileWrite, None) => None,
        (NextMarker::DoneAfterFileWrite, Some(_)) | (NextMarker::DoneAlways, _) => {
            Some("next: done".to_string())
        }
        (NextMarker::ApplyCommand, None) => Some("next: kubectl apply -f -".to_string()),
        (NextMarker::ApplyCommand, Some(path)) => {
            Some(format!("next: kubectl apply -f {}", path.display()))
        }
    }
}

/// How `render_operator` replaces the checked-in namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamespaceRule {
    /// Replace only the `name: <ns>` and `namespace: <ns>` lines.
    KubernetesLines { checked_in: String },
    /// Replace every occurrence of the checked-in text.
    ReplaceAll { checked_in: String },
}

/// The texts of an operator render.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperatorParts {
    /// The manifest texts (rbac, deployment, ...).
    pub parts: Vec<String>,
    /// The optional monitoring texts.
    pub extras: Vec<String>,
    /// `(from, to_template)` pairs applied to the extras; `{namespace}` in
    /// `to_template` is replaced by the namespace.
    pub extra_rewrites: Vec<(String, String)>,
}

/// An input that `render_operator` or its validators refuse.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The namespace is not a valid lowercase DNS label.
    InvalidNamespace,
    /// The image is not an OCI image reference.
    InvalidImage,
    /// The named token is empty, too long, or has whitespace or control characters.
    InvalidToken {
        /// The name of the token.
        name: String,
    },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::InvalidNamespace => {
                write!(f, "operator namespace must be a valid lowercase DNS label")
            }
            RenderError::InvalidImage => write!(
                f,
                "operator image contains characters outside an OCI image reference"
            ),
            RenderError::InvalidToken { name } => {
                write!(f, "{name} must be a nonempty single bounded token")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// `Ok` for a lowercase DNS label of 1 to 63 bytes: `a-z`, `0-9`, `-`, not
/// starting or ending with `-`.
pub fn validate_namespace(ns: &str) -> Result<(), RenderError> {
    let bytes = ns.as_bytes();
    let ok = (1..=63).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && bytes.first() != Some(&b'-')
        && bytes.last() != Some(&b'-');
    if ok {
        Ok(())
    } else {
        Err(RenderError::InvalidNamespace)
    }
}

/// `Ok` for a nonempty image of at most 255 bytes with only ASCII letters,
/// digits and `._-:/@+`.
pub fn validate_image(image: &str) -> Result<(), RenderError> {
    let ok = !image.is_empty()
        && image.len() <= 255
        && image
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-:/@+".contains(&b));
    if ok {
        Ok(())
    } else {
        Err(RenderError::InvalidImage)
    }
}

/// `Ok` for a nonempty, at most 512-byte value with no whitespace or control
/// characters.
pub fn validate_token(name: &str, value: &str) -> Result<(), RenderError> {
    let bad = value.trim().is_empty()
        || value.len() > 512
        || value.chars().any(|c| c.is_control() || c.is_whitespace());
    if bad {
        Err(RenderError::InvalidToken {
            name: name.to_string(),
        })
    } else {
        Ok(())
    }
}

/// The CRD text: the operator's CRD when given, else the embedded one, with a
/// trailing newline.
pub fn render_crd(operator_crd: Option<&str>, embedded: &str) -> String {
    match operator_crd {
        Some(text) => text.to_string(),
        None if embedded.ends_with('\n') => embedded.to_string(),
        None => format!("{embedded}\n"),
    }
}

/// Remove source-ownership markers.
// Same rules as interfaces::artifact; the domain layer cannot call interfaces.
fn strip_source_ownership_markers(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("# SPEC-MANAGED:")
            || trimmed == "# CODEGEN-BEGIN"
            || trimmed == "# CODEGEN-END"
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Substitute the namespace in the `name:` and `namespace:` lines.
// Same rules as interfaces::artifact; the domain layer cannot call interfaces.
fn replace_kubernetes_namespace(input: &str, checked_in: &str, namespace: &str) -> String {
    input
        .replace(
            &format!("name: {checked_in}"),
            &format!("name: {namespace}"),
        )
        .replace(
            &format!("namespace: {checked_in}"),
            &format!("namespace: {namespace}"),
        )
}

/// The operator render: strip the markers, join the parts (and the extras
/// when `with_extras`) with `---`, then apply the namespace rule and the image.
pub fn render_operator(
    p: &OperatorParts,
    with_extras: bool,
    rule: &NamespaceRule,
    namespace: &str,
    image: Option<(&str, &str)>,
) -> Result<String, RenderError> {
    validate_namespace(namespace)?;
    if let Some((from, to)) = image {
        validate_token("operator image source", from)?;
        validate_image(to)?;
    }
    let mut texts: Vec<String> = p
        .parts
        .iter()
        .map(|t| strip_source_ownership_markers(t))
        .collect();
    if with_extras {
        for extra in &p.extras {
            let mut text = strip_source_ownership_markers(extra);
            for (from, to_template) in &p.extra_rewrites {
                let to = to_template.replace("{namespace}", namespace);
                text = text.replace(from.as_str(), &to);
            }
            texts.push(text);
        }
    }
    let joined = texts.join("\n---\n");
    let result = match rule {
        NamespaceRule::KubernetesLines { checked_in } => {
            replace_kubernetes_namespace(&joined, checked_in, namespace)
        }
        NamespaceRule::ReplaceAll { checked_in } => joined.replace(checked_in.as_str(), namespace),
    };
    Ok(match image {
        Some((from, to)) => result.replace(from, to),
        None => result,
    })
}

/// A profile of `instance render`. The index order of the spec's arrays is
/// Dev, Staging, Prod, Template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Dev,
    Staging,
    Prod,
    Template,
}

impl Profile {
    /// Parses an exact, lowercase profile name.
    pub fn parse(s: &str) -> Option<Profile> {
        match s {
            "dev" => Some(Profile::Dev),
            "staging" => Some(Profile::Staging),
            "prod" => Some(Profile::Prod),
            "template" => Some(Profile::Template),
            _ => None,
        }
    }

    /// The profile name as the CLI spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::Dev => "dev",
            Profile::Staging => "staging",
            Profile::Prod => "prod",
            Profile::Template => "template",
        }
    }

    fn index(self) -> usize {
        match self {
            Profile::Dev => 0,
            Profile::Staging => 1,
            Profile::Prod => 2,
            Profile::Template => 3,
        }
    }
}

/// The optional `instance render` inputs; `None` takes the spec's value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceInputs {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub image: Option<String>,
}

/// The per-service parameters of `instance render`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceSpec {
    pub api_group: String,
    pub kind: String,
    pub default_name: String,
    pub profile_namespaces: [String; 4],
    pub profile_images: [String; 4],
    pub template_name: Option<String>,
}

/// Renders one instance manifest: the fixed header, then the service's spec
/// body for the profile, unchanged.
pub fn render_instance(
    spec: &InstanceSpec,
    profile: Profile,
    inputs: &InstanceInputs,
    body: &dyn Fn(Profile) -> String,
) -> String {
    let index = profile.index();
    let template = if profile == Profile::Template {
        spec.template_name.as_ref()
    } else {
        None
    };
    let name = inputs
        .name
        .clone()
        .or_else(|| template.cloned())
        .unwrap_or_else(|| spec.default_name.clone());
    let namespace = inputs
        .namespace
        .clone()
        .unwrap_or_else(|| spec.profile_namespaces[index].clone());
    let image = inputs
        .image
        .clone()
        .unwrap_or_else(|| spec.profile_images[index].clone());
    format!(
        "apiVersion: {}/v1alpha1\nkind: {}\nmetadata:\n  name: {}\n  namespace: {}\nspec:\n  image: {}\n{}",
        spec.api_group,
        spec.kind,
        name,
        namespace,
        image,
        body(profile)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn for_service_names() {
        let names = DefaultFileNames::for_service("tape");
        assert_eq!(names.crd, "crd.yaml");
        assert_eq!(names.operator, "operator.yaml");
        assert_eq!(names.instance, "tape.yaml");
        assert_eq!(names.collector, None);
    }

    #[test]
    fn prefixed_names() {
        let names = DefaultFileNames::prefixed("sift");
        assert_eq!(names.crd, "sift-crd.yaml");
        assert_eq!(names.operator, "sift-operator.yaml");
        assert_eq!(names.instance, "sift.yaml");
        assert_eq!(names.collector, Some("sift-collector.yaml".to_string()));
    }

    #[test]
    fn file_for_maps_each_artifact() {
        let sift = DefaultFileNames::prefixed("sift");
        assert_eq!(sift.file_for(Artifact::Crd), Some("sift-crd.yaml"));
        assert_eq!(
            sift.file_for(Artifact::Operator),
            Some("sift-operator.yaml")
        );
        assert_eq!(sift.file_for(Artifact::Instance), Some("sift.yaml"));
        assert_eq!(
            sift.file_for(Artifact::Collector),
            Some("sift-collector.yaml")
        );
        let tape = DefaultFileNames::for_service("tape");
        assert_eq!(tape.file_for(Artifact::Collector), None);
    }

    #[test]
    fn next_line_none_is_always_none() {
        assert_eq!(
            next_line(NextMarker::None, Some(Path::new("/tmp/x.yaml"))),
            None
        );
        assert_eq!(next_line(NextMarker::None, None), None);
    }

    #[test]
    fn next_line_done_after_file_write() {
        assert_eq!(
            next_line(
                NextMarker::DoneAfterFileWrite,
                Some(Path::new("/tmp/x.yaml"))
            ),
            Some("next: done".to_string())
        );
        assert_eq!(next_line(NextMarker::DoneAfterFileWrite, None), None);
    }

    #[test]
    fn next_line_done_always() {
        assert_eq!(
            next_line(NextMarker::DoneAlways, None),
            Some("next: done".to_string())
        );
        assert_eq!(
            next_line(NextMarker::DoneAlways, Some(Path::new("/tmp/x.yaml"))),
            Some("next: done".to_string())
        );
    }

    #[test]
    fn next_line_apply_command() {
        assert_eq!(
            next_line(NextMarker::ApplyCommand, None),
            Some("next: kubectl apply -f -".to_string())
        );
        assert_eq!(
            next_line(NextMarker::ApplyCommand, Some(Path::new("/tmp/x.yaml"))),
            Some("next: kubectl apply -f /tmp/x.yaml".to_string())
        );
    }

    const A: &str = "# SPEC-MANAGED: x\nname: tape-system\nkind: A\n";
    const B: &str = "# SPEC-MANAGED: x\nnamespace: tape-system\nkind: B\n";

    fn kubernetes_rule() -> NamespaceRule {
        NamespaceRule::KubernetesLines {
            checked_in: "tape-system".to_string(),
        }
    }

    fn two_parts() -> OperatorParts {
        OperatorParts {
            parts: vec![A.to_string(), B.to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn render_crd_some_is_returned_unchanged() {
        assert_eq!(render_crd(Some("a: 1"), "b: 2"), "a: 1");
    }

    #[test]
    fn render_crd_none_adds_trailing_newline() {
        assert_eq!(render_crd(None, "b: 2"), "b: 2\n");
        assert_eq!(render_crd(None, "b: 2\n"), "b: 2\n");
    }

    #[test]
    fn render_operator_joins_parts_and_strips_markers() {
        let out = render_operator(&two_parts(), false, &kubernetes_rule(), "staging", None);
        assert_eq!(
            out,
            Ok("name: staging\nkind: A\n\n---\nnamespace: staging\nkind: B\n".to_string())
        );
        assert!(!out.unwrap().contains("tape-system"));
    }

    #[test]
    fn render_operator_extras_only_when_asked() {
        let mut p = two_parts();
        p.extras = vec!["# SPEC-MANAGED: m\nkind: Mon\nhost: tape-system.svc\n".to_string()];
        p.extra_rewrites = vec![("tape-system.svc".to_string(), "{namespace}.svc".to_string())];
        let with = render_operator(&p, true, &kubernetes_rule(), "staging", None);
        assert_eq!(
            with,
            Ok("name: staging\nkind: A\n\n---\nnamespace: staging\nkind: B\n\n---\nkind: Mon\nhost: staging.svc\n".to_string())
        );
        let without = render_operator(&p, false, &kubernetes_rule(), "staging", None);
        assert_eq!(
            without,
            Ok("name: staging\nkind: A\n\n---\nnamespace: staging\nkind: B\n".to_string())
        );
    }

    #[test]
    fn kubernetes_lines_rule_leaves_other_text() {
        let p = OperatorParts {
            parts: vec!["value: sift-system.svc\n".to_string()],
            ..Default::default()
        };
        let rule = NamespaceRule::KubernetesLines {
            checked_in: "sift-system".to_string(),
        };
        assert_eq!(
            render_operator(&p, false, &rule, "prod", None),
            Ok("value: sift-system.svc\n".to_string())
        );
    }

    #[test]
    fn replace_all_rule_replaces_every_occurrence() {
        let p = OperatorParts {
            parts: vec!["value: sift-system.svc\n".to_string()],
            ..Default::default()
        };
        let rule = NamespaceRule::ReplaceAll {
            checked_in: "sift-system".to_string(),
        };
        assert_eq!(
            render_operator(&p, false, &rule, "prod", None),
            Ok("value: prod.svc\n".to_string())
        );
    }

    #[test]
    fn image_is_replaced_after_validation() {
        let p = OperatorParts {
            parts: vec!["image: example/op:1\n".to_string()],
            ..Default::default()
        };
        let out = render_operator(
            &p,
            false,
            &kubernetes_rule(),
            "staging",
            Some(("example/op:1", "ghcr.io/x/op:2")),
        );
        assert_eq!(out, Ok("image: ghcr.io/x/op:2\n".to_string()));
    }

    #[test]
    fn invalid_namespace_is_rejected() {
        for bad in ["Bad_NS", "", "-a", "a-"] {
            assert_eq!(validate_namespace(bad), Err(RenderError::InvalidNamespace));
        }
        assert_eq!(
            validate_namespace(&"a".repeat(64)),
            Err(RenderError::InvalidNamespace)
        );
        for good in ["staging", "a-1"] {
            assert_eq!(validate_namespace(good), Ok(()));
        }
        assert_eq!(validate_namespace(&"a".repeat(63)), Ok(()));
        assert_eq!(
            render_operator(&two_parts(), false, &kubernetes_rule(), "Bad_NS", None),
            Err(RenderError::InvalidNamespace)
        );
    }

    #[test]
    fn invalid_image_is_rejected() {
        assert_eq!(validate_image("a b"), Err(RenderError::InvalidImage));
        assert_eq!(validate_image(""), Err(RenderError::InvalidImage));
        assert_eq!(
            validate_image(&"a".repeat(256)),
            Err(RenderError::InvalidImage)
        );
        assert_eq!(validate_image("ghcr.io/org/op:1.2@sha256:ab+x"), Ok(()));
        assert_eq!(
            render_operator(
                &two_parts(),
                false,
                &kubernetes_rule(),
                "staging",
                Some(("example/op:1", "a b")),
            ),
            Err(RenderError::InvalidImage)
        );
    }

    #[test]
    fn invalid_token_is_rejected() {
        let src = || RenderError::InvalidToken {
            name: "operator image source".to_string(),
        };
        assert_eq!(validate_token("operator image source", ""), Err(src()));
        assert_eq!(validate_token("operator image source", "a b"), Err(src()));
        assert_eq!(validate_token("operator image source", "a\tb"), Err(src()));
        assert_eq!(
            validate_token("operator image source", &"a".repeat(513)),
            Err(src())
        );
        assert_eq!(validate_token("x", "ok"), Ok(()));
        assert_eq!(
            render_operator(
                &two_parts(),
                false,
                &kubernetes_rule(),
                "staging",
                Some(("", "a:1")),
            ),
            Err(src())
        );
    }

    #[test]
    fn render_error_messages() {
        assert_eq!(
            RenderError::InvalidNamespace.to_string(),
            "operator namespace must be a valid lowercase DNS label"
        );
        assert_eq!(
            RenderError::InvalidImage.to_string(),
            "operator image contains characters outside an OCI image reference"
        );
        assert_eq!(
            RenderError::InvalidToken {
                name: "tag".to_string()
            }
            .to_string(),
            "tag must be a nonempty single bounded token"
        );
    }

    fn instance_spec(template_name: Option<&str>) -> InstanceSpec {
        InstanceSpec {
            api_group: "example.dev".to_string(),
            kind: "Tape".to_string(),
            default_name: "tape".to_string(),
            profile_namespaces: [
                "default".to_string(),
                "staging".to_string(),
                "production".to_string(),
                "REPLACE_ME__APP_NAMESPACE".to_string(),
            ],
            profile_images: [
                "img/dev:1".to_string(),
                "img/staging:1".to_string(),
                "img/prod:1".to_string(),
                "REPLACE_ME__IMAGE".to_string(),
            ],
            template_name: template_name.map(str::to_string),
        }
    }

    fn instance_body(profile: Profile) -> String {
        match profile {
            Profile::Template => "  replicasPerShard: REPLACE_ME__REPLICAS_PER_SHARD\n".to_string(),
            _ => "  replicasPerShard: 1\n".to_string(),
        }
    }

    #[test]
    fn instance_dev_exact_yaml() {
        let out = render_instance(
            &instance_spec(None),
            Profile::Dev,
            &InstanceInputs::default(),
            &instance_body,
        );
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: tape\n  namespace: default\nspec:\n  image: img/dev:1\n  replicasPerShard: 1\n"
        );
    }

    #[test]
    fn instance_dev_has_no_placeholder() {
        let out = render_instance(
            &instance_spec(None),
            Profile::Dev,
            &InstanceInputs::default(),
            &instance_body,
        );
        assert!(!out.contains("REPLACE_ME"));
    }

    #[test]
    fn instance_template_uses_placeholders() {
        let out = render_instance(
            &instance_spec(None),
            Profile::Template,
            &InstanceInputs::default(),
            &instance_body,
        );
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: tape\n  namespace: REPLACE_ME__APP_NAMESPACE\nspec:\n  image: REPLACE_ME__IMAGE\n  replicasPerShard: REPLACE_ME__REPLICAS_PER_SHARD\n"
        );
    }

    #[test]
    fn instance_inputs_override() {
        let inputs = InstanceInputs {
            name: Some("n1".to_string()),
            namespace: Some("ns1".to_string()),
            image: Some("i1".to_string()),
        };
        let out = render_instance(
            &instance_spec(None),
            Profile::Staging,
            &inputs,
            &instance_body,
        );
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: n1\n  namespace: ns1\nspec:\n  image: i1\n  replicasPerShard: 1\n"
        );

        let only_ns = InstanceInputs {
            name: None,
            namespace: Some("ns2".to_string()),
            image: None,
        };
        let out = render_instance(
            &instance_spec(None),
            Profile::Prod,
            &only_ns,
            &instance_body,
        );
        assert!(out.contains("  name: tape\n"));
        assert!(out.contains("  namespace: ns2\n"));
        assert!(out.contains("  image: img/prod:1\n"));
    }

    #[test]
    fn instance_template_name_only_for_template() {
        let spec = instance_spec(Some("REPLACE_ME__KEEP_NAME"));
        let template = render_instance(
            &spec,
            Profile::Template,
            &InstanceInputs::default(),
            &instance_body,
        );
        assert!(template.contains("  name: REPLACE_ME__KEEP_NAME\n"));

        let dev = render_instance(
            &spec,
            Profile::Dev,
            &InstanceInputs::default(),
            &instance_body,
        );
        assert!(dev.contains("  name: tape\n"));

        let named = InstanceInputs {
            name: Some("n1".to_string()),
            namespace: None,
            image: None,
        };
        let template_named = render_instance(&spec, Profile::Template, &named, &instance_body);
        assert!(template_named.contains("  name: n1\n"));
    }

    #[test]
    fn profile_parse_and_as_str() {
        assert_eq!(Profile::parse("prod"), Some(Profile::Prod));
        assert_eq!(Profile::parse("dev"), Some(Profile::Dev));
        assert_eq!(Profile::parse("staging"), Some(Profile::Staging));
        assert_eq!(Profile::parse("template"), Some(Profile::Template));
        assert_eq!(Profile::parse("x"), None);
        assert_eq!(Profile::parse("Dev"), None);
        assert_eq!(Profile::parse(""), None);

        assert_eq!(Profile::Dev.as_str(), "dev");
        assert_eq!(Profile::Staging.as_str(), "staging");
        assert_eq!(Profile::Prod.as_str(), "prod");
        assert_eq!(Profile::Template.as_str(), "template");
        for p in [
            Profile::Dev,
            Profile::Staging,
            Profile::Prod,
            Profile::Template,
        ] {
            assert_eq!(Profile::parse(p.as_str()), Some(p));
        }
    }
}
