//! [`RenderCtx`]: the per-service identity every render helper threads through.

use serde_json::{json, Value};

/// Per-service render identity, threaded through the helpers.
///
/// Built with [`RenderCtx::new`], whose six `&str` parameters come in the
/// order of the getters below; [`RenderCtx::with_owner`] adds the owner
/// reference that [`RenderCtx::meta`] attaches to every child.
pub struct RenderCtx<'a> {
    app: &'a str,
    manager: &'a str,
    api_version: &'a str,
    kind: &'a str,
    name: &'a str,
    ns: &'a str,
    owner: Option<Value>,
    instance_uid: Option<String>,
}

impl<'a> RenderCtx<'a> {
    /// A context with no owner reference. The arguments are all `&str`, so
    /// keep them in this order: app, field manager, the owner CR's
    /// apiVersion and kind, the instance name, the namespace.
    pub fn new(
        app: &'a str,
        manager: &'a str,
        api_version: &'a str,
        kind: &'a str,
        name: &'a str,
        ns: &'a str,
    ) -> Self {
        Self {
            app,
            manager,
            api_version,
            kind,
            name,
            ns,
            owner: None,
            instance_uid: None,
        }
    }

    /// Set the owner reference (see [`super::owner_ref`]) that
    /// [`Self::meta`] attaches to every child. Takes a `Value` or an
    /// `Option<Value>`; `None` leaves the context without an owner.
    pub fn with_owner(mut self, owner: impl Into<Option<Value>>) -> Self {
        self.owner = owner.into();
        self
    }

    /// The UID of the owner CR. It is stamped on the volume claim templates of
    /// a StatefulSet so a later Delete can prove that a PVC belongs to this
    /// instance (#517). `labels()`, `selector()` and `meta()` do not use it.
    pub fn with_instance_uid(mut self, uid: impl Into<String>) -> Self {
        self.instance_uid = Some(uid.into());
        self
    }

    /// The owner CR's UID, if one was set with [`Self::with_instance_uid`].
    pub fn instance_uid(&self) -> Option<&str> {
        self.instance_uid.as_deref()
    }

    /// `app.kubernetes.io/name` and `part-of`.
    pub fn app(&self) -> &'a str {
        self.app
    }

    /// `app.kubernetes.io/managed-by`.
    pub fn manager(&self) -> &'a str {
        self.manager
    }

    /// The owner CR's apiVersion.
    pub fn api_version(&self) -> &'a str {
        self.api_version
    }

    /// The owner CR's kind.
    pub fn kind(&self) -> &'a str {
        self.kind
    }

    /// The instance name, `app.kubernetes.io/instance`.
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// The namespace every child is rendered into.
    pub fn ns(&self) -> &'a str {
        self.ns
    }

    /// The owner reference, if one was set.
    pub fn owner(&self) -> Option<&Value> {
        self.owner.as_ref()
    }
}

impl RenderCtx<'_> {
    /// Recommended labels common to every child object.
    pub fn labels(&self, component: &str) -> Value {
        json!({
            "app.kubernetes.io/name": self.app,
            "app.kubernetes.io/instance": self.name,
            "app.kubernetes.io/component": component,
            "app.kubernetes.io/managed-by": self.manager,
            "app.kubernetes.io/part-of": self.app,
        })
    }

    /// Immutable selector labels (a subset of [`Self::labels`]) — workload and
    /// Service selectors pin to these so re-applies never hit selector-immutability.
    pub fn selector(&self, component: &str) -> Value {
        json!({
            "app.kubernetes.io/name": self.app,
            "app.kubernetes.io/instance": self.name,
            "app.kubernetes.io/component": component,
        })
    }

    /// Assemble an object's `metadata` block (name/ns/labels + owner ref).
    pub fn meta(&self, name: &str, component: &str) -> Value {
        let mut m = json!({ "name": name, "namespace": self.ns, "labels": self.labels(component) });
        if let Some(o) = &self.owner {
            m["ownerReferences"] = json!([o]);
        }
        m
    }
}
