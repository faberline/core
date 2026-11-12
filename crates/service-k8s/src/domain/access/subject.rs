//! The identity a grant binds to: one ServiceAccount.

/// A ServiceAccount that receives the rules of an [`AccessGrant`](super::AccessGrant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessSubject {
    pub namespace: String,
    pub name: String,
}

impl AccessSubject {
    /// A subject named by the namespace and the ServiceAccount name.
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }
}
