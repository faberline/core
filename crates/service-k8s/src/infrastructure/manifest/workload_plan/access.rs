//! RBAC plans: Roles, RoleBindings and cluster-scoped ClusterRoleBindings.

use serde_json::{json, Value};

use super::{LabelSet, RenderCtx};
use crate::infrastructure::manifest::rbac;

/// One rule of a [`RolePlan`].
#[derive(Clone, Debug)]
pub struct RbacRulePlan {
    api_groups: Vec<String>,
    resources: Vec<String>,
    resource_names: Vec<String>,
    verbs: Vec<String>,
}

impl RbacRulePlan {
    /// A rule over every object of `resources`; narrow it to named objects
    /// with [`Self::with_resource_names`].
    pub fn new(api_groups: Vec<String>, resources: Vec<String>, verbs: Vec<String>) -> Self {
        Self {
            api_groups,
            resources,
            resource_names: Vec::new(),
            verbs,
        }
    }

    /// Add names to `resourceNames`. The rule renders `resourceNames` only
    /// when at least one name is set.
    pub fn with_resource_names(mut self, names: Vec<String>) -> Self {
        self.resource_names.extend(names);
        self
    }

    pub fn api_groups(&self) -> &[String] {
        &self.api_groups
    }

    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    pub fn resource_names(&self) -> &[String] {
        &self.resource_names
    }

    pub fn verbs(&self) -> &[String] {
        &self.verbs
    }
}

/// A namespaced Role, owned by the custom resource like every other child.
#[derive(Clone, Debug)]
pub struct RolePlan {
    name: String,
    component: String,
    rules: Vec<RbacRulePlan>,
}

impl RolePlan {
    /// A Role with no rules yet; add them with [`Self::with_rule`].
    pub fn new(name: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            rules: Vec::new(),
        }
    }

    pub fn with_rule(mut self, rule: RbacRulePlan) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn rules(&self) -> &[RbacRulePlan] {
        &self.rules
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let rules = self
            .rules
            .into_iter()
            .map(|rule| {
                let mut value = json!({
                    "apiGroups": rule.api_groups,
                    "resources": rule.resources,
                    "verbs": rule.verbs,
                });
                if !rule.resource_names.is_empty() {
                    value["resourceNames"] = json!(rule.resource_names);
                }
                value
            })
            .collect::<Vec<_>>();
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "Role",
            "metadata": cx.meta(&self.name, &self.component),
            "rules": rules,
        })
    }
}

/// A ServiceAccount subject of a [`RoleBindingPlan`] or
/// [`ClusterRoleBindingPlan`].
#[derive(Clone, Debug)]
pub struct ServiceAccountSubjectPlan {
    name: String,
    namespace: String,
}

impl ServiceAccountSubjectPlan {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: namespace.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

/// A namespaced RoleBinding from ServiceAccounts to a Role.
#[derive(Clone, Debug)]
pub struct RoleBindingPlan {
    name: String,
    component: String,
    role_name: String,
    subjects: Vec<ServiceAccountSubjectPlan>,
}

impl RoleBindingPlan {
    /// A binding to the Role `role_name` with no subjects yet; add them with
    /// [`Self::with_service_account`].
    pub fn new(
        name: impl Into<String>,
        component: impl Into<String>,
        role_name: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            role_name: role_name.into(),
            subjects: Vec::new(),
        }
    }

    pub fn with_service_account(mut self, subject: ServiceAccountSubjectPlan) -> Self {
        self.subjects.push(subject);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn role_name(&self) -> &str {
        &self.role_name
    }

    pub fn subjects(&self) -> &[ServiceAccountSubjectPlan] {
        &self.subjects
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "RoleBinding",
            "metadata": cx.meta(&self.name, &self.component),
            "roleRef": {
                "apiGroup": "rbac.authorization.k8s.io",
                "kind": "Role",
                "name": self.role_name,
            },
            "subjects": self.subjects.into_iter().map(|subject| json!({
                "kind": "ServiceAccount",
                "name": subject.name,
                "namespace": subject.namespace,
            })).collect::<Vec<_>>(),
        })
    }
}

/// A cluster-scoped binding to an existing ClusterRole.
///
/// Kubernetes does not allow a namespaced custom resource to own a
/// cluster-scoped child. This plan therefore renders labels, but never a
/// namespace or owner reference. Callers must include strong owner labels and
/// return the same object from [`crate::service::ManagedService::cluster_scoped_children`].
/// The shared controller then installs a finalizer and controls cleanup.
#[derive(Clone, Debug)]
pub struct ClusterRoleBindingPlan {
    pub name: String,
    pub component: String,
    pub cluster_role: String,
    pub subjects: Vec<ServiceAccountSubjectPlan>,
    pub labels: LabelSet,
}

impl ClusterRoleBindingPlan {
    pub fn new(
        name: impl Into<String>,
        component: impl Into<String>,
        cluster_role: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            component: component.into(),
            cluster_role: cluster_role.into(),
            subjects: Vec::new(),
            labels: LabelSet::new(),
        }
    }

    pub fn with_service_account(mut self, subject: ServiceAccountSubjectPlan) -> Self {
        self.subjects.push(subject);
        self
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    pub(super) fn render(self, cx: &RenderCtx<'_>) -> Value {
        let mut labels = cx.labels(&self.component);
        let label_object = labels
            .as_object_mut()
            .expect("RenderCtx::labels always returns a JSON object");
        for (key, value) in self.labels {
            label_object.insert(key, Value::String(value));
        }
        let subjects = self
            .subjects
            .iter()
            .map(|subject| rbac::ServiceAccountSubject {
                namespace: &subject.namespace,
                name: &subject.name,
            })
            .collect::<Vec<_>>();
        rbac::cluster_role_binding(rbac::ClusterRoleBinding {
            name: &self.name,
            labels,
            cluster_role: &self.cluster_role,
            subjects: &subjects,
        })
    }
}

#[cfg(feature = "controller")]
use crate::domain::access::AccessGrant;

/// Render `grants` as a sequence of Role and RoleBinding Kubernetes manifests.
///
/// Returns, for each grant in (namespace, name) order, one Role and one RoleBinding
/// built with the existing `rbac::role` / `rbac::role_binding` builders. The
/// RoleBinding binds that Role (never a ClusterRole), uses ServiceAccount subjects
/// only, and carries no owner reference. Equal input gives equal output regardless
/// of input order.
#[cfg(feature = "controller")]
pub fn render_access_grants(grants: &[AccessGrant]) -> Vec<Value> {
    let mut sorted = grants.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| {
        (a.namespace.as_str(), a.name.as_str()).cmp(&(b.namespace.as_str(), b.name.as_str()))
    });

    let mut result = Vec::with_capacity(sorted.len() * 2);

    for grant in sorted {
        let api_groups_vec: Vec<Vec<&str>> = grant
            .rules
            .iter()
            .map(|r| r.api_groups.iter().map(String::as_str).collect())
            .collect();
        let resources_vec: Vec<Vec<&str>> = grant
            .rules
            .iter()
            .map(|r| r.resources.iter().map(String::as_str).collect())
            .collect();
        let resource_names_vec: Vec<Vec<&str>> = grant
            .rules
            .iter()
            .map(|r| r.resource_names.iter().map(String::as_str).collect())
            .collect();
        let verbs_vec: Vec<Vec<&str>> = grant
            .rules
            .iter()
            .map(|r| r.verbs.iter().map(String::as_str).collect())
            .collect();

        let rules: Vec<rbac::NamedRule<'_>> = (0..grant.rules.len())
            .map(|i| rbac::NamedRule {
                api_groups: &api_groups_vec[i],
                resources: &resources_vec[i],
                resource_names: &resource_names_vec[i],
                verbs: &verbs_vec[i],
            })
            .collect();

        let role_val = rbac::role(rbac::Role {
            name: &grant.name,
            namespace: &grant.namespace,
            labels: json!({}),
            rules: &rules,
        });

        let sa_subjects: Vec<rbac::ServiceAccountSubject<'_>> = grant
            .subjects
            .iter()
            .map(|s| rbac::ServiceAccountSubject {
                namespace: &s.namespace,
                name: &s.name,
            })
            .collect();

        let role_subjects: Vec<rbac::RoleSubject<'_>> = sa_subjects
            .iter()
            .map(|sa| {
                rbac::RoleSubject::ServiceAccount(rbac::ServiceAccountSubject {
                    namespace: sa.namespace,
                    name: sa.name,
                })
            })
            .collect();

        let binding_val = rbac::role_binding(rbac::RoleBinding {
            name: &grant.name,
            namespace: &grant.namespace,
            labels: json!({}),
            role: &grant.name,
            subjects: &role_subjects,
        });

        result.push(role_val);
        result.push(binding_val);
    }

    result
}
