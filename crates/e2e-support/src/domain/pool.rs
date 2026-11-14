/// A pool that workloads must run on: a node selector and an image pull secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolPlacement {
    pub key: String,
    pub value: String,
    pub pull_secret: String,
}

impl PoolPlacement {
    /// `None` when `pod` (a pod spec holder: JSON with `spec`) is on the pool, else the error text.
    pub fn placement_error(&self, pod: &serde_json::Value) -> Option<String> {
        use serde_json::Value;
        let spec = &pod["spec"];
        let actual = spec["nodeSelector"].get(self.key.as_str());
        if actual.and_then(Value::as_str) != Some(self.value.as_str()) {
            let shown = match actual {
                Some(v) => v.as_str().map_or_else(|| v.to_string(), str::to_string),
                None => "null".to_string(),
            };
            return Some(format!(
                "pod is not on pool {}={}: nodeSelector is {shown}",
                self.key, self.value
            ));
        }
        let has_secret = spec["imagePullSecrets"].as_array().is_some_and(|secrets| {
            secrets
                .iter()
                .any(|s| s["name"].as_str() == Some(self.pull_secret.as_str()))
        });
        if !has_secret {
            return Some(format!("pod has no imagePullSecret {}", self.pull_secret));
        }
        None
    }

    pub fn assert_on_pool(&self, pod: &serde_json::Value) {
        if let Some(e) = self.placement_error(pod) {
            panic!("{e}");
        }
    }

    /// Checks `workload["spec"]["template"]` (a Deployment, StatefulSet or Job).
    pub fn workload_placement_error(
        &self,
        what: &str,
        workload: &serde_json::Value,
    ) -> Option<String> {
        self.placement_error(&workload["spec"]["template"])
            .map(|e| format!("{what}: {e}"))
    }

    pub fn assert_pool_placement(&self, what: &str, workload: &serde_json::Value) {
        if let Some(e) = self.workload_placement_error(what, workload) {
            panic!("{e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn pool() -> PoolPlacement {
        PoolPlacement {
            key: "pool".into(),
            value: "e2e".into(),
            pull_secret: "regcred".into(),
        }
    }

    fn on_pool() -> Value {
        json!({"spec": {"nodeSelector": {"pool": "e2e"}, "imagePullSecrets": [{"name": "regcred"}]}})
    }

    fn off_pool() -> Value {
        json!({"spec": {"nodeSelector": {"pool": "other"}, "imagePullSecrets": [{"name": "regcred"}]}})
    }

    #[test]
    fn on_pool_pod_passes() {
        assert_eq!(pool().placement_error(&on_pool()), None);
    }

    #[test]
    fn off_pool_pod_fails_with_the_pool_key() {
        // Negative control: a wrong nodeSelector must fail.
        let e = pool().placement_error(&off_pool()).expect("error");
        assert!(e.contains("pool=e2e"), "{e}");
    }

    #[test]
    fn missing_pull_secret_fails() {
        let no_secret = json!({"spec": {"nodeSelector": {"pool": "e2e"}}});
        let e = pool().placement_error(&no_secret).expect("error");
        assert!(e.contains("regcred"), "{e}");
    }

    #[test]
    #[should_panic(expected = "not on pool")]
    fn assert_on_pool_panics_off_pool() {
        pool().assert_on_pool(&off_pool());
    }

    #[test]
    fn workload_placement_uses_the_template_and_prefixes_what() {
        let p = pool();
        assert_eq!(
            p.workload_placement_error("sts/x", &json!({"spec": {"template": on_pool()}})),
            None
        );
        let e = p
            .workload_placement_error("sts/x", &json!({"spec": {"template": off_pool()}}))
            .expect("error");
        assert!(e.starts_with("sts/x: "), "{e}");
    }
}
