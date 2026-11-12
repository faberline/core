//! A named set of rules granted to a set of ServiceAccounts in one namespace.

use super::{AccessRule, AccessSubject};

/// One access grant: a Role with `rules` and a RoleBinding to `subjects`,
/// both named `name` in `namespace`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessGrant {
    pub namespace: String,
    pub name: String,
    pub rules: Vec<AccessRule>,
    pub subjects: Vec<AccessSubject>,
}

impl AccessGrant {
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        rules: Vec<AccessRule>,
        subjects: Vec<AccessSubject>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            rules,
            subjects,
        }
    }
}
