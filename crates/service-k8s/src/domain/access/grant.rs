//! App-declared access grant model.

use super::{rule::AccessRule, subject::AccessSubject};

/// A typed grant declared by an application.
///
/// For each grant, the library renders one Role and one RoleBinding in
/// `namespace`.
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
