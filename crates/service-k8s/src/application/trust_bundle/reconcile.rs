//! Reconciliation of public CA trust bundles across declared client namespaces.

use serde_json::json;
use std::collections::BTreeMap;

use crate::domain::condition::{ConditionFact, ConditionStatus};
use crate::domain::trust_bundle::{
    canonicalize_bundle_pem, client_trust_ready_condition, validate_public_certificates,
    ConfigMapStore, NamespaceOutcome, TrustBundleSpec, CA_BUNDLE_DATA_KEY,
    OWNER_IDENTITY_ANNOTATION, OWNER_IDENTITY_LABEL,
};

/// The report produced by a trust bundle reconciliation pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustBundleReport {
    /// Outcomes indexed by namespace.
    pub outcomes: BTreeMap<String, NamespaceOutcome>,
    /// The `ClientTrustReady` condition fact reflecting convergence state.
    pub condition: ConditionFact,
    /// Whether all operations succeeded and client trust is ready.
    pub is_ready: bool,
}

/// Construct the desired ConfigMap JSON representation.
fn build_config_map_json(
    spec: &TrustBundleSpec,
    namespace: &str,
    pem_data: &str,
) -> serde_json::Value {
    let mut labels = spec.labels.clone();
    labels.insert(
        OWNER_IDENTITY_LABEL.to_string(),
        spec.owner_identity.clone(),
    );

    let mut annotations = BTreeMap::new();
    annotations.insert(
        OWNER_IDENTITY_ANNOTATION.to_string(),
        spec.owner_identity.clone(),
    );

    let mut data = BTreeMap::new();
    data.insert(CA_BUNDLE_DATA_KEY.to_string(), pem_data.to_string());

    json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": spec.config_map_name,
            "namespace": namespace,
            "labels": labels,
            "annotations": annotations,
        },
        "data": data,
    })
}

/// Reconciles public trust bundles according to `spec` using `store`.
///
/// Refuses out-of-scope operations and foreign owners, isolates per-namespace errors,
/// prunes stale owned ConfigMaps, and reports the `ClientTrustReady` condition.
pub async fn reconcile(store: &dyn ConfigMapStore, spec: &TrustBundleSpec) -> TrustBundleReport {
    // 1. Validate public certificate bytes
    let certs = match validate_public_certificates(&spec.ca_bundle_pem) {
        Ok(certs) => certs,
        Err(err) => {
            let condition = client_trust_ready_condition(&BTreeMap::new(), Some(&err.to_string()));
            return TrustBundleReport {
                outcomes: BTreeMap::new(),
                condition,
                is_ready: false,
            };
        }
    };
    let canonical_pem = canonicalize_bundle_pem(&certs);

    let mut outcomes = BTreeMap::new();

    // 2. Converge in declared allowed namespaces
    for ns in &spec.allowed_namespaces {
        match store.read(ns, &spec.config_map_name).await {
            Err(err) => {
                outcomes.insert(
                    ns.clone(),
                    NamespaceOutcome::Failed {
                        error: err.to_string(),
                    },
                );
            }
            Ok(Some(cm)) => {
                let existing_owner = cm
                    .annotations
                    .get(OWNER_IDENTITY_ANNOTATION)
                    .or_else(|| cm.labels.get(OWNER_IDENTITY_LABEL));

                match existing_owner {
                    Some(owner) if owner == &spec.owner_identity => {
                        let desired = build_config_map_json(spec, ns, &canonical_pem);
                        match store.apply(ns, &spec.config_map_name, desired).await {
                            Ok(()) => {
                                outcomes.insert(ns.clone(), NamespaceOutcome::Applied);
                            }
                            Err(err) => {
                                outcomes.insert(
                                    ns.clone(),
                                    NamespaceOutcome::Failed {
                                        error: err.to_string(),
                                    },
                                );
                            }
                        }
                    }
                    Some(foreign_owner) => {
                        outcomes.insert(
                            ns.clone(),
                            NamespaceOutcome::AdoptionRefused {
                                foreign_owner: foreign_owner.clone(),
                            },
                        );
                    }
                    None => {
                        if spec.adopt_unowned {
                            let desired = build_config_map_json(spec, ns, &canonical_pem);
                            match store.apply(ns, &spec.config_map_name, desired).await {
                                Ok(()) => {
                                    outcomes.insert(ns.clone(), NamespaceOutcome::Adopted);
                                }
                                Err(err) => {
                                    outcomes.insert(
                                        ns.clone(),
                                        NamespaceOutcome::Failed {
                                            error: err.to_string(),
                                        },
                                    );
                                }
                            }
                        } else {
                            outcomes.insert(ns.clone(), NamespaceOutcome::AdoptionRefusedUnowned);
                        }
                    }
                }
            }
            Ok(None) => {
                let desired = build_config_map_json(spec, ns, &canonical_pem);
                match store.apply(ns, &spec.config_map_name, desired).await {
                    Ok(()) => {
                        outcomes.insert(ns.clone(), NamespaceOutcome::Applied);
                    }
                    Err(err) => {
                        outcomes.insert(
                            ns.clone(),
                            NamespaceOutcome::Failed {
                                error: err.to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    // 3. Prune candidate namespaces no longer allowed
    for ns in &spec.prune_candidates {
        if spec.allowed_namespaces.contains(ns) {
            continue;
        }

        match store.read(ns, &spec.config_map_name).await {
            Err(err) => {
                outcomes.insert(
                    ns.clone(),
                    NamespaceOutcome::Failed {
                        error: err.to_string(),
                    },
                );
            }
            Ok(Some(cm)) => {
                let existing_owner = cm
                    .annotations
                    .get(OWNER_IDENTITY_ANNOTATION)
                    .or_else(|| cm.labels.get(OWNER_IDENTITY_LABEL));

                match existing_owner {
                    Some(owner) if owner == &spec.owner_identity => {
                        match store.delete(ns, &spec.config_map_name).await {
                            Ok(()) => {
                                outcomes.insert(ns.clone(), NamespaceOutcome::Pruned);
                            }
                            Err(err) => {
                                outcomes.insert(
                                    ns.clone(),
                                    NamespaceOutcome::Failed {
                                        error: err.to_string(),
                                    },
                                );
                            }
                        }
                    }
                    _ => {
                        outcomes.insert(ns.clone(), NamespaceOutcome::ForeignRetained);
                    }
                }
            }
            Ok(None) => {}
        }
    }

    let condition = client_trust_ready_condition(&outcomes, None);
    let is_ready = condition.status == ConditionStatus::True;

    TrustBundleReport {
        outcomes,
        condition,
        is_ready,
    }
}
