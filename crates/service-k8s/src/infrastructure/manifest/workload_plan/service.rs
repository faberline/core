//! ServiceAccount and Service plans.

use serde_json::{json, Value};

use super::{merge_string_labels, LabelSet, RenderCtx};

#[derive(Clone, Debug)]
pub struct ServiceAccountPlan {
    pub name: String,
    pub component: String,
    pub automount_service_account_token: bool,
}

impl ServiceAccountPlan {
    pub fn new(name: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            automount_service_account_token: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServicePortPlan {
    pub name: String,
    pub port: i32,
    pub target_port: String,
    pub protocol: String,
}

impl ServicePortPlan {
    pub fn tcp(name: impl Into<String>, port: i32, target_port: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            port,
            target_port: target_port.into(),
            protocol: "TCP".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServicePlan {
    pub name: String,
    pub component: String,
    pub selector_component: String,
    pub selector: LabelSet,
    pub ports: Vec<ServicePortPlan>,
    pub cluster_ip: Option<String>,
    pub publish_not_ready_addresses: bool,
    pub service_type: Option<String>,
}

impl ServicePlan {
    pub fn cluster_ip(
        name: impl Into<String>,
        component: impl Into<String>,
        selector: LabelSet,
        ports: Vec<ServicePortPlan>,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            selector_component: String::new(),
            selector,
            ports,
            cluster_ip: None,
            publish_not_ready_addresses: false,
            service_type: Some("ClusterIP".into()),
        }
        .with_default_selector_component()
    }

    pub fn headless(
        name: impl Into<String>,
        component: impl Into<String>,
        selector: LabelSet,
        ports: Vec<ServicePortPlan>,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            selector_component: String::new(),
            selector,
            ports,
            cluster_ip: Some("None".into()),
            publish_not_ready_addresses: true,
            service_type: None,
        }
        .with_default_selector_component()
    }

    fn with_default_selector_component(mut self) -> Self {
        self.selector_component = self.component.clone();
        self
    }

    pub fn with_selector_component(mut self, component: impl Into<String>) -> Self {
        self.selector_component = component.into();
        self
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let ports = self
            .ports
            .into_iter()
            .map(|port| {
                json!({
                    "name": port.name,
                    "port": port.port,
                    "targetPort": port.target_port,
                    "protocol": port.protocol,
                })
            })
            .collect::<Vec<_>>();
        let selector = merge_string_labels(cx.selector(&self.selector_component), &self.selector);
        let mut spec = json!({"selector": selector, "ports": ports});
        if let Some(cluster_ip) = self.cluster_ip {
            spec["clusterIP"] = json!(cluster_ip);
        }
        if self.publish_not_ready_addresses {
            spec["publishNotReadyAddresses"] = json!(true);
        }
        if let Some(service_type) = self.service_type {
            spec["type"] = json!(service_type);
        }
        json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": cx.meta(&self.name, &self.component),
            "spec": spec,
        })
    }
}
