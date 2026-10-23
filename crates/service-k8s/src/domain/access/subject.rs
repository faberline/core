//! ServiceAccount access subject.

/// A ServiceAccount subject of an [`super::AccessGrant`], restricted to a
/// namespace and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessSubject {
    pub namespace: String,
    pub name: String,
}

impl AccessSubject {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }
}
