//! Pod runtime policy, containers and the pod template they render into.

use serde_json::{json, Value};

use super::{merge_string_labels, LabelSet, RenderCtx};

#[derive(Clone, Debug)]
pub struct PodRuntimePolicy {
    pub service_account_name: String,
    pub automount_service_account_token: bool,
    pub enable_service_links: bool,
    pub termination_grace_period_seconds: u64,
    pub security_context: Value,
    pub node_selector: Value,
    pub affinity: Option<Value>,
    pub init_containers: Vec<Value>,
    pub volumes: Vec<Value>,
    pub restart_policy: Option<String>,
}

impl PodRuntimePolicy {
    /// Restricted non-root baseline for stateful and stateless services.
    pub fn restricted(service_account_name: impl Into<String>, node_selector: Value) -> Self {
        Self {
            service_account_name: service_account_name.into(),
            automount_service_account_token: false,
            enable_service_links: false,
            termination_grace_period_seconds: 30,
            security_context: json!({
                "runAsNonRoot": true,
                "runAsUser": 65532,
                "runAsGroup": 65532,
                "fsGroup": 65532,
                "fsGroupChangePolicy": "OnRootMismatch",
                "seccompProfile": {"type": "RuntimeDefault"},
            }),
            node_selector,
            affinity: None,
            init_containers: Vec::new(),
            volumes: Vec::new(),
            restart_policy: None,
        }
    }

    pub fn with_automount_service_account_token(mut self, value: bool) -> Self {
        self.automount_service_account_token = value;
        self
    }

    pub fn with_affinity(mut self, value: Value) -> Self {
        self.affinity = Some(value);
        self
    }

    pub fn with_init_containers(mut self, values: Vec<Value>) -> Self {
        self.init_containers = values;
        self
    }

    pub fn with_volumes(mut self, values: Vec<Value>) -> Self {
        self.volumes = values;
        self
    }

    pub fn with_restart_policy(mut self, value: impl Into<String>) -> Self {
        self.restart_policy = Some(value.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct ContainerPlan {
    pub name: String,
    pub image: String,
    pub image_pull_policy: Option<String>,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub ports: Vec<Value>,
    pub env: Vec<Value>,
    pub env_from: Vec<Value>,
    pub volume_mounts: Vec<Value>,
    pub security_context: Option<Value>,
    pub resources: Option<Value>,
    pub readiness_probe: Option<Value>,
    pub liveness_probe: Option<Value>,
    pub startup_probe: Option<Value>,
    pub lifecycle: Option<Value>,
}

impl ContainerPlan {
    pub fn new(name: impl Into<String>, image: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            image: image.into(),
            image_pull_policy: None,
            command: Vec::new(),
            args,
            ports: Vec::new(),
            env: Vec::new(),
            env_from: Vec::new(),
            volume_mounts: Vec::new(),
            security_context: None,
            resources: None,
            readiness_probe: None,
            liveness_probe: None,
            startup_probe: None,
            lifecycle: None,
        }
    }

    fn render(mut self) -> Value {
        let mut value = json!({
            "name": self.name,
            "image": self.image,
            "args": self.args,
            "ports": self.ports,
            "env": self.env,
            "volumeMounts": self.volume_mounts,
        });
        if let Some(policy) = self.image_pull_policy.take() {
            value["imagePullPolicy"] = json!(policy);
        }
        if !self.command.is_empty() {
            value["command"] = json!(self.command);
        }
        if !self.env_from.is_empty() {
            value["envFrom"] = json!(self.env_from);
        }
        for (key, field) in [
            ("securityContext", self.security_context),
            ("resources", self.resources),
            ("readinessProbe", self.readiness_probe),
            ("livenessProbe", self.liveness_probe),
            ("startupProbe", self.startup_probe),
            ("lifecycle", self.lifecycle),
        ] {
            if let Some(field) = field {
                value[key] = field;
            }
        }
        value
    }
}

#[derive(Clone, Debug)]
pub struct PodPlan {
    pub component: String,
    pub selector_labels: LabelSet,
    pub labels: LabelSet,
    pub container: ContainerPlan,
    pub runtime: PodRuntimePolicy,
}

impl PodPlan {
    pub fn new(
        component: impl Into<String>,
        container: ContainerPlan,
        runtime: PodRuntimePolicy,
    ) -> Self {
        Self {
            component: component.into(),
            selector_labels: LabelSet::new(),
            labels: LabelSet::new(),
            container,
            runtime,
        }
    }

    pub fn with_selector_labels(mut self, labels: LabelSet) -> Self {
        self.selector_labels.extend(labels.clone());
        self.labels.extend(labels);
        self
    }

    pub fn with_labels(mut self, labels: LabelSet) -> Self {
        self.labels.extend(labels);
        self
    }

    pub(super) fn selector(&self, cx: &RenderCtx<'_>) -> Value {
        merge_string_labels(cx.selector(&self.component), &self.selector_labels)
    }

    fn labels(&self, cx: &RenderCtx<'_>) -> Value {
        merge_string_labels(cx.labels(&self.component), &self.labels)
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let labels = self.labels(cx);
        let mut spec = json!({
            "serviceAccountName": self.runtime.service_account_name,
            "automountServiceAccountToken": self.runtime.automount_service_account_token,
            "enableServiceLinks": self.runtime.enable_service_links,
            "terminationGracePeriodSeconds": self.runtime.termination_grace_period_seconds,
            "securityContext": self.runtime.security_context,
            "containers": [self.container.render()],
        });
        if self.runtime.node_selector != json!({}) {
            spec["nodeSelector"] = self.runtime.node_selector;
        }
        if let Some(affinity) = self.runtime.affinity {
            spec["affinity"] = affinity;
        }
        if !self.runtime.init_containers.is_empty() {
            spec["initContainers"] = json!(self.runtime.init_containers);
        }
        if !self.runtime.volumes.is_empty() {
            spec["volumes"] = json!(self.runtime.volumes);
        }
        if let Some(restart_policy) = self.runtime.restart_policy {
            spec["restartPolicy"] = json!(restart_policy);
        }
        json!({"metadata": {"labels": labels}, "spec": spec})
    }
}
