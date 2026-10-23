//! Typed rule of an access grant.

/// One rule of an [`super::AccessGrant`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessRule {
    pub api_groups: Vec<String>,
    pub resources: Vec<String>,
    pub resource_names: Vec<String>,
    pub verbs: Vec<String>,
}

impl AccessRule {
    pub fn new(
        api_groups: Vec<String>,
        resources: Vec<String>,
        resource_names: Vec<String>,
        verbs: Vec<String>,
    ) -> Self {
        Self {
            api_groups,
            resources,
            resource_names,
            verbs,
        }
    }
}
