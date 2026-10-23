//! Specification for public CA trust bundle convergence across client namespaces.

use std::collections::{BTreeMap, BTreeSet};

/// Annotation identifying the owning manager/service of the ConfigMap.
pub const OWNER_IDENTITY_ANNOTATION: &str = "service-k8s.axiom.dev/owner-identity";

/// Label identifying the owning manager/service of the ConfigMap.
pub const OWNER_IDENTITY_LABEL: &str = "service-k8s.axiom.dev/owner-identity";

/// Default key within the ConfigMap `.data` holding concatenated PEM CA certificates.
pub const CA_BUNDLE_DATA_KEY: &str = "ca.crt";

/// Inputs for converging public CA trust bundles in declared client namespaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustBundleSpec {
    /// The unique identity of the owning controller/service (e.g. `"lumen"`).
    pub owner_identity: String,
    /// Name of the ConfigMap to publish in each allowed namespace.
    pub config_map_name: String,
    /// Raw PEM certificate bundle data.
    pub ca_bundle_pem: String,
    /// Additional metadata labels to apply to created or updated ConfigMaps.
    pub labels: BTreeMap<String, String>,
    /// Set of namespaces where the trust bundle ConfigMap must be published.
    pub allowed_namespaces: BTreeSet<String>,
    /// Set of candidate namespaces checked for stale owned ConfigMaps to prune.
    pub prune_candidates: BTreeSet<String>,
    /// Whether to adopt existing ConfigMaps that have no owner identity.
    pub adopt_unowned: bool,
}

impl TrustBundleSpec {
    /// Create a new spec with required parameters and default options.
    pub fn new(
        owner_identity: impl Into<String>,
        config_map_name: impl Into<String>,
        ca_bundle_pem: impl Into<String>,
    ) -> Self {
        Self {
            owner_identity: owner_identity.into(),
            config_map_name: config_map_name.into(),
            ca_bundle_pem: ca_bundle_pem.into(),
            labels: BTreeMap::new(),
            allowed_namespaces: BTreeSet::new(),
            prune_candidates: BTreeSet::new(),
            adopt_unowned: false,
        }
    }

    /// Add custom metadata labels.
    pub fn with_labels(mut self, labels: BTreeMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }

    /// Add a single custom metadata label.
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Add a single allowed namespace.
    pub fn with_allowed_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.allowed_namespaces.insert(namespace.into());
        self
    }

    /// Add multiple allowed namespaces.
    pub fn with_allowed_namespaces(
        mut self,
        namespaces: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        for ns in namespaces {
            self.allowed_namespaces.insert(ns.into());
        }
        self
    }

    /// Add client ServiceAccount targets, extracting unique namespaces.
    ///
    /// Multiple ServiceAccounts in the same namespace share the same desired ConfigMap.
    pub fn with_service_accounts<N, S>(
        mut self,
        service_accounts: impl IntoIterator<Item = (N, S)>,
    ) -> Self
    where
        N: Into<String>,
        S: Into<String>,
    {
        for (ns, _) in service_accounts {
            self.allowed_namespaces.insert(ns.into());
        }
        self
    }

    /// Specify candidate namespaces to check for prune.
    pub fn with_prune_candidates(
        mut self,
        namespaces: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        for ns in namespaces {
            self.prune_candidates.insert(ns.into());
        }
        self
    }

    /// Set whether unowned ConfigMaps are adopted or refused.
    pub fn with_adopt_unowned(mut self, adopt: bool) -> Self {
        self.adopt_unowned = adopt;
        self
    }
}
