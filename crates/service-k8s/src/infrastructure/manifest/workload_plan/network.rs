//! NetworkPolicy and FQDN egress-policy plans.

use serde_json::{json, Value};

use super::{merge_string_labels, LabelSet, RenderCtx};

#[derive(Clone, Debug)]
pub struct NetworkPortPlan {
    pub protocol: String,
    pub port: i32,
}

impl NetworkPortPlan {
    pub fn tcp(port: i32) -> Self {
        Self {
            protocol: "TCP".into(),
            port,
        }
    }

    pub fn udp(port: i32) -> Self {
        Self {
            protocol: "UDP".into(),
            port,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NetworkPeerPlan {
    Any,
    SameNamespace,
    Pods {
        namespace: Option<String>,
        selector: LabelSet,
    },
    IpBlock {
        cidr: String,
        except: Vec<String>,
    },
}

impl NetworkPeerPlan {
    pub fn any() -> Self {
        Self::Any
    }

    pub fn same_namespace_pods(selector: LabelSet) -> Self {
        Self::Pods {
            namespace: None,
            selector,
        }
    }

    pub fn pods_in_namespace(namespace: impl Into<String>, selector: LabelSet) -> Self {
        Self::Pods {
            namespace: Some(namespace.into()),
            selector,
        }
    }

    pub fn ip_block(cidr: impl Into<String>) -> Self {
        Self::IpBlock {
            cidr: cidr.into(),
            except: Vec::new(),
        }
    }

    fn render(&self, namespace: &str) -> Value {
        match self {
            Self::Any => json!({}),
            Self::SameNamespace => json!({
                "namespaceSelector": {"matchLabels": {"kubernetes.io/metadata.name": namespace}},
            }),
            Self::Pods {
                namespace: selected_namespace,
                selector,
            } => json!({
                "namespaceSelector": {"matchLabels": {
                    "kubernetes.io/metadata.name": selected_namespace.as_deref().unwrap_or(namespace)
                }},
                "podSelector": {"matchLabels": selector},
            }),
            Self::IpBlock { cidr, except } => {
                let mut block = json!({"cidr": cidr});
                if !except.is_empty() {
                    block["except"] = json!(except);
                }
                json!({"ipBlock": block})
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkRulePlan {
    pub peers: Vec<NetworkPeerPlan>,
    pub ports: Vec<NetworkPortPlan>,
}

impl NetworkRulePlan {
    pub fn new(peers: Vec<NetworkPeerPlan>, ports: Vec<NetworkPortPlan>) -> Self {
        Self { peers, ports }
    }

    fn render(&self, direction: &str, namespace: &str) -> Value {
        let unrestricted = self
            .peers
            .iter()
            .any(|peer| matches!(peer, NetworkPeerPlan::Any));
        let ports = self
            .ports
            .iter()
            .map(|port| json!({"protocol": port.protocol, "port": port.port}))
            .collect::<Vec<_>>();
        let mut value = json!({"ports": ports});
        if !unrestricted {
            value[direction] = json!(self
                .peers
                .iter()
                .map(|peer| peer.render(namespace))
                .collect::<Vec<_>>());
        }
        value
    }
}

#[derive(Clone, Debug)]
pub enum FqdnMatchPlan {
    Name(String),
    Pattern(String),
}

impl FqdnMatchPlan {
    pub fn name(value: impl Into<String>) -> Self {
        Self::Name(value.into())
    }

    pub fn pattern(value: impl Into<String>) -> Self {
        Self::Pattern(value.into())
    }

    fn render(self) -> Value {
        match self {
            Self::Name(name) => json!({"name": name}),
            Self::Pattern(pattern) => json!({"pattern": pattern}),
        }
    }
}

/// GKE Dataplane V2 external-egress allowlist by DNS name.
#[derive(Clone, Debug)]
pub struct FqdnNetworkPolicyPlan {
    pub name: String,
    pub component: String,
    pub selector: LabelSet,
    pub matches: Vec<FqdnMatchPlan>,
    pub ports: Vec<NetworkPortPlan>,
}

impl FqdnNetworkPolicyPlan {
    pub fn new(name: impl Into<String>, component: impl Into<String>, selector: LabelSet) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            selector,
            matches: Vec::new(),
            ports: Vec::new(),
        }
    }

    pub fn with_match(mut self, matcher: FqdnMatchPlan) -> Self {
        self.matches.push(matcher);
        self
    }

    pub fn with_port(mut self, port: NetworkPortPlan) -> Self {
        self.ports.push(port);
        self
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        json!({
            "apiVersion": "networking.gke.io/v1alpha1",
            "kind": "FQDNNetworkPolicy",
            "metadata": cx.meta(&self.name, &self.component),
            "spec": {
                "podSelector": {"matchLabels": self.selector},
                "egress": [{
                    "matches": self.matches.into_iter().map(FqdnMatchPlan::render).collect::<Vec<_>>(),
                    "ports": self.ports.into_iter().map(|port| json!({
                        "protocol": port.protocol,
                        "port": port.port,
                    })).collect::<Vec<_>>(),
                }],
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct NetworkPolicyPlan {
    pub name: String,
    pub component: String,
    pub selector: LabelSet,
    pub ingress: Vec<NetworkRulePlan>,
    pub egress: Vec<NetworkRulePlan>,
    pub instance_wide: bool,
}

impl NetworkPolicyPlan {
    pub fn new(name: impl Into<String>, component: impl Into<String>, selector: LabelSet) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            selector,
            ingress: Vec::new(),
            egress: Vec::new(),
            instance_wide: false,
        }
    }

    pub fn with_ingress(mut self, rule: NetworkRulePlan) -> Self {
        self.ingress.push(rule);
        self
    }

    pub fn with_egress(mut self, rule: NetworkRulePlan) -> Self {
        self.egress.push(rule);
        self
    }

    /// Select every pod in this managed instance. This is used only for the
    /// base default-deny policy. Role policies should keep their narrow
    /// component selector.
    pub fn instance_wide(mut self) -> Self {
        self.instance_wide = true;
        self
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let selector = if self.instance_wide {
            merge_string_labels(
                json!({
                    "app.kubernetes.io/name": cx.app(),
                    "app.kubernetes.io/instance": cx.name(),
                }),
                &self.selector,
            )
        } else {
            merge_string_labels(cx.selector(&self.component), &self.selector)
        };
        let ingress = self
            .ingress
            .iter()
            .map(|rule| rule.render("from", cx.ns()))
            .collect::<Vec<_>>();
        let egress = self
            .egress
            .iter()
            .map(|rule| rule.render("to", cx.ns()))
            .collect::<Vec<_>>();
        json!({
            "apiVersion": "networking.k8s.io/v1",
            "kind": "NetworkPolicy",
            "metadata": cx.meta(&self.name, &self.component),
            "spec": {
                "podSelector": {"matchLabels": selector},
                "policyTypes": ["Ingress", "Egress"],
                "ingress": ingress,
                "egress": egress,
            },
        })
    }
}
