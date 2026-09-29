use super::cache::CachePolicy;

/// A configuration that cannot describe a safe delegated authenticator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingAudience;

impl std::fmt::Display for MissingAudience {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "DelegatedAuthConfig::new requires at least one audience; use the explicitly named \
             kubernetes_default constructor only when default Kubernetes ServiceAccount tokens \
             are the intended caller credential",
        )
    }
}

impl std::error::Error for MissingAudience {}

/// What this service will accept, and for how long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedAuthConfig {
    pub(crate) audiences: Vec<String>,
    /// An explicit opt-in to TokenReview's Kubernetes-default audience mode.
    /// This can never be reached through [`Self::new`].
    pub(crate) kubernetes_default: bool,
    /// Cache TTLs and the stale window. See [`CachePolicy`].
    pub cache: CachePolicy,
}

impl DelegatedAuthConfig {
    /// Build a configuration. At least one audience is mandatory: a
    /// `TokenReview` with an empty audience list validates against the
    /// apiserver's own audience, which means every pod's default token in the
    /// cluster would authenticate here.
    pub fn new(audiences: Vec<String>) -> Result<Self, MissingAudience> {
        let audiences: Vec<String> = audiences
            .into_iter()
            .filter(|audience| !audience.is_empty())
            .collect();
        if audiences.is_empty() {
            return Err(MissingAudience);
        }
        Ok(Self {
            audiences,
            kubernetes_default: false,
            cache: CachePolicy::default(),
        })
    }

    /// Accept the default ServiceAccount token mounted by Kubernetes.
    ///
    /// This deliberately asks TokenReview to use the apiserver's configured
    /// audiences. It is for in-cluster services whose public contract says a
    /// caller's default KSA identity is the credential. Services that mint a
    /// private audience must continue to use [`Self::new`].
    pub fn kubernetes_default() -> Self {
        Self {
            audiences: Vec::new(),
            kubernetes_default: true,
            cache: CachePolicy::default(),
        }
    }

    /// The audiences to put in TokenReview. Empty is meaningful only when
    /// [`Self::uses_kubernetes_default`] is true; the kube backend then omits
    /// `spec.audiences` rather than sending an empty array.
    pub fn audiences(&self) -> &[String] {
        &self.audiences
    }

    pub fn uses_kubernetes_default(&self) -> bool {
        self.kubernetes_default
    }

    pub fn with_cache_policy(mut self, cache: CachePolicy) -> Self {
        self.cache = cache;
        self
    }
}
