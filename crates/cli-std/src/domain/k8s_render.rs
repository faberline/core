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
}
