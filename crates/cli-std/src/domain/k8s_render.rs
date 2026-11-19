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

/// The profile of a rendered service instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// The development profile.
    Dev,
    /// The staging profile.
    Staging,
    /// The production profile.
    Prod,
    /// The template profile, with placeholders.
    Template,
}

impl Profile {
    /// The profile named `s` (`dev`, `staging`, `prod`, `template`); `None` otherwise.
    pub fn parse(s: &str) -> Option<Profile> {
        match s {
            "dev" => Some(Profile::Dev),
            "staging" => Some(Profile::Staging),
            "prod" => Some(Profile::Prod),
            "template" => Some(Profile::Template),
            _ => None,
        }
    }

    /// The name of the profile.
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::Dev => "dev",
            Profile::Staging => "staging",
            Profile::Prod => "prod",
            Profile::Template => "template",
        }
    }

    /// The position of the profile in the per-profile arrays of `InstanceSpec`.
    fn index(self) -> usize {
        match self {
            Profile::Dev => 0,
            Profile::Staging => 1,
            Profile::Prod => 2,
            Profile::Template => 3,
        }
    }
}

/// The values a user gives for one instance; `None` takes the spec's value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceInputs {
    /// The instance name, when given.
    pub name: Option<String>,
    /// The namespace, when given.
    pub namespace: Option<String>,
    /// The image, when given.
    pub image: Option<String>,
}

/// What a service says about its instance: the API group, the kind, the names,
/// and the per-profile namespaces and images (index 0 Dev, 1 Staging, 2 Prod,
/// 3 Template).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceSpec {
    /// The API group, without the version.
    pub api_group: String,
    /// The kind of the instance.
    pub kind: String,
    /// The instance name when no input or template name gives one.
    pub default_name: String,
    /// The namespace of each profile.
    pub profile_namespaces: [String; 4],
    /// The image of each profile.
    pub profile_images: [String; 4],
    /// The name used for `Profile::Template`, when the service has one.
    pub template_name: Option<String>,
}

/// Renders one service instance: the header, then `body(profile)` as given.
pub fn render_instance(
    spec: &InstanceSpec,
    profile: Profile,
    inputs: &InstanceInputs,
    body: &dyn Fn(Profile) -> String,
) -> String {
    let template_name = match profile {
        Profile::Template => spec.template_name.clone(),
        _ => None,
    };
    let name = inputs
        .name
        .clone()
        .or(template_name)
        .unwrap_or_else(|| spec.default_name.clone());
    let namespace = inputs
        .namespace
        .clone()
        .unwrap_or_else(|| spec.profile_namespaces[profile.index()].clone());
    let image = inputs
        .image
        .clone()
        .unwrap_or_else(|| spec.profile_images[profile.index()].clone());
    format!(
        "apiVersion: {}/v1alpha1\nkind: {}\nmetadata:\n  name: {name}\n  namespace: {namespace}\nspec:\n  image: {image}\n{}",
        spec.api_group,
        spec.kind,
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

    fn spec(template_name: Option<&str>) -> InstanceSpec {
        InstanceSpec {
            api_group: "example.dev".to_string(),
            kind: "Tape".to_string(),
            default_name: "tape".to_string(),
            profile_namespaces: [
                "default",
                "staging",
                "production",
                "REPLACE_ME__APP_NAMESPACE",
            ]
            .map(String::from),
            profile_images: [
                "img/dev:1",
                "img/staging:1",
                "img/prod:1",
                "REPLACE_ME__IMAGE",
            ]
            .map(String::from),
            template_name: template_name.map(String::from),
        }
    }

    fn body(profile: Profile) -> String {
        if profile == Profile::Template {
            "  replicasPerShard: REPLACE_ME__REPLICAS_PER_SHARD\n".to_string()
        } else {
            "  replicasPerShard: 1\n".to_string()
        }
    }

    #[test]
    fn instance_dev_exact_yaml() {
        let out = render_instance(&spec(None), Profile::Dev, &InstanceInputs::default(), &body);
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: tape\n  namespace: default\nspec:\n  image: img/dev:1\n  replicasPerShard: 1\n"
        );
    }

    #[test]
    fn instance_dev_has_no_placeholder() {
        let out = render_instance(&spec(None), Profile::Dev, &InstanceInputs::default(), &body);
        assert!(!out.contains("REPLACE_ME"));
    }

    #[test]
    fn instance_template_uses_placeholders() {
        let out = render_instance(
            &spec(None),
            Profile::Template,
            &InstanceInputs::default(),
            &body,
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
        let out = render_instance(&spec(None), Profile::Staging, &inputs, &body);
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: n1\n  namespace: ns1\nspec:\n  image: i1\n  replicasPerShard: 1\n"
        );

        let only_ns = InstanceInputs {
            namespace: Some("ns2".to_string()),
            ..Default::default()
        };
        let out = render_instance(&spec(None), Profile::Prod, &only_ns, &body);
        assert_eq!(
            out,
            "apiVersion: example.dev/v1alpha1\nkind: Tape\nmetadata:\n  name: tape\n  namespace: ns2\nspec:\n  image: img/prod:1\n  replicasPerShard: 1\n"
        );
    }

    #[test]
    fn instance_template_name_only_for_template() {
        let s = spec(Some("REPLACE_ME__KEEP_NAME"));
        let template = render_instance(&s, Profile::Template, &InstanceInputs::default(), &body);
        assert!(template.contains("  name: REPLACE_ME__KEEP_NAME\n"));

        let dev = render_instance(&s, Profile::Dev, &InstanceInputs::default(), &body);
        assert!(dev.contains("  name: tape\n"));

        let named = InstanceInputs {
            name: Some("n1".to_string()),
            ..Default::default()
        };
        let template_named = render_instance(&s, Profile::Template, &named, &body);
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
