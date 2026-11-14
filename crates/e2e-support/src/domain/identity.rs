use super::names::is_lower_hex;
use serde_json::Value;

/// True when `s` is `sha256:` followed by 64 lower hex chars.
fn is_digest(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(|hex| is_lower_hex(hex, 64))
}

/// The build identity that a deployed workload must run: image, digests, version and git sha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub image: String,
    pub root_digest: String,
    pub runtime_digest: String,
    pub version: String,
    pub git_sha: String,
    allow_single_manifest: bool,
}

impl Identity {
    /// Checks the arguments and builds the identity. `Err` names the bad argument.
    pub fn new(
        image_repo: &str,
        root_digest: &str,
        runtime_digest: &str,
        version: &str,
        git_sha: &str,
        allow_single_manifest: bool,
    ) -> Result<Self, String> {
        if image_repo.is_empty() {
            return Err("image_repo is empty".to_string());
        }
        if !is_digest(root_digest) {
            return Err(format!(
                "root_digest is not sha256:<64 lower hex>: {root_digest}"
            ));
        }
        if !is_digest(runtime_digest) {
            return Err(format!(
                "runtime_digest is not sha256:<64 lower hex>: {runtime_digest}"
            ));
        }
        if version.is_empty() {
            return Err("version is empty".to_string());
        }
        if !is_lower_hex(git_sha, 40) {
            return Err(format!("git_sha is not 40 lower hex chars: {git_sha}"));
        }
        if runtime_digest == root_digest && !allow_single_manifest {
            return Err(
                "runtime_digest equals root_digest: a single manifest is not allowed".to_string(),
            );
        }
        Ok(Self {
            image: format!("{image_repo}@{root_digest}"),
            root_digest: root_digest.to_string(),
            runtime_digest: runtime_digest.to_string(),
            version: version.to_string(),
            git_sha: git_sha.to_string(),
            allow_single_manifest,
        })
    }

    pub fn version_error(&self, got: &str) -> Option<String> {
        (got != self.version)
            .then(|| format!("version mismatch: expected {}, got {got}", self.version))
    }

    pub fn assert_version(&self, got: &str) {
        if let Some(e) = self.version_error(got) {
            panic!("{e}");
        }
    }

    pub fn template_image_error(&self, workload: &Value) -> Option<String> {
        let Some(containers) = workload["spec"]["template"]["spec"]["containers"].as_array() else {
            return Some("workload has no containers".to_string());
        };
        for c in containers {
            let image = c.get("image").and_then(Value::as_str).unwrap_or("");
            if image != self.image {
                let name = c.get("name").and_then(Value::as_str).unwrap_or("");
                return Some(format!(
                    "container {name} has image {image}, expected {}",
                    self.image
                ));
            }
        }
        None
    }

    pub fn assert_template_image(&self, workload: &Value) {
        if let Some(e) = self.template_image_error(workload) {
            panic!("{e}");
        }
    }

    pub fn pods_error(&self, pods: &Value) -> Option<String> {
        use super::image::normalize_runtime_image_id;
        let Some(items) = pods["items"].as_array().filter(|i| !i.is_empty()) else {
            return Some("no pods".to_string());
        };
        for pod in items {
            let name = pod["metadata"]["name"].as_str().unwrap_or("");
            let Some(statuses) = pod["status"]["containerStatuses"]
                .as_array()
                .filter(|s| !s.is_empty())
            else {
                return Some(format!("pod {name} has no containerStatuses"));
            };
            for s in statuses {
                let container = s["name"].as_str().unwrap_or("");
                let raw = s["imageID"].as_str().unwrap_or("");
                if normalize_runtime_image_id(raw) != Some(self.runtime_digest.as_str()) {
                    return Some(format!(
                        "pod {name} container {container} runs {raw}, expected {}",
                        self.runtime_digest
                    ));
                }
            }
        }
        None
    }

    pub fn assert_pods(&self, pods: &Value) {
        if let Some(e) = self.pods_error(pods) {
            panic!("{e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    fn e() -> String {
        format!("sha256:{}", "b".repeat(64))
    }

    fn id() -> Identity {
        Identity::new("reg.io/app", &d(), &e(), "1.2.3", &"c".repeat(40), false)
            .expect("valid identity")
    }

    #[test]
    fn new_builds_the_image_reference() {
        assert_eq!(id().image, format!("reg.io/app@{}", d()));
    }

    #[test]
    fn new_rejects_bad_arguments() {
        let (dd, ee, c40) = (d(), e(), "c".repeat(40));
        assert!(Identity::new("", &dd, &ee, "1.2.3", &c40, false).is_err());
        assert!(Identity::new("reg.io/app", "sha256:short", &ee, "1.2.3", &c40, false).is_err());
        assert!(Identity::new("reg.io/app", &dd, &ee, "1.2.3", &"c".repeat(39), false).is_err());
        assert!(Identity::new("reg.io/app", &dd, &ee, "", &c40, false).is_err());
        // Negative control: one manifest only with the explicit allowance.
        assert!(Identity::new("reg.io/app", &dd, &dd, "1.2.3", &c40, false).is_err());
        assert!(Identity::new("reg.io/app", &dd, &dd, "1.2.3", &c40, true).is_ok());
    }

    #[test]
    fn version_error_names_both_versions() {
        let id = id();
        assert_eq!(id.version_error("1.2.3"), None);
        let e = id.version_error("1.2.4").expect("error");
        assert!(e.contains("expected 1.2.3"), "{e}");
    }

    #[test]
    fn template_image_must_be_the_identity_image() {
        let id = id();
        let ok = json!({"spec": {"template": {"spec": {"containers": [{"name": "app", "image": id.image}]}}}});
        assert_eq!(id.template_image_error(&ok), None);
        let bad = json!({"spec": {"template": {"spec": {"containers": [{"name": "app", "image": "reg.io/app:latest"}]}}}});
        let e = id.template_image_error(&bad).expect("error");
        assert!(e.contains("app:latest"), "{e}");
        let none = json!({"spec": {"template": {"spec": {}}}});
        assert_eq!(
            id.template_image_error(&none),
            Some("workload has no containers".to_string())
        );
    }

    #[test]
    fn pods_must_run_the_runtime_digest() {
        let id = id();
        let pod = |image_id: &str| json!({"items": [{"metadata": {"name": "p1"}, "status": {"containerStatuses": [{"name": "app", "imageID": image_id}]}}]});
        assert_eq!(id.pods_error(&pod(&format!("cri-o://{}", e()))), None);
        // Negative control: the root digest is not the runtime digest.
        let wrong = id
            .pods_error(&pod(&format!("cri-o://{}", d())))
            .expect("error");
        assert!(wrong.contains("expected"), "{wrong}");
        assert_eq!(
            id.pods_error(&json!({"items": []})),
            Some("no pods".to_string())
        );
        assert!(id.pods_error(&pod("garbage")).is_some());
    }
}
