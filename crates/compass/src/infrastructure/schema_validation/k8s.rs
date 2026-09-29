//! Programmatic K8s JSON-Schema definitions for common resources.
//!
//! Each function returns `(kind_name, schema_value)`.  The schemas cover the
//! most commonly misconfigured required-field constraints so that `K8002`
//! catches real issues without bundling the full 15 MB upstream schemas.

use serde_json::{json, Value};

/// Build schemas for all supported resource kinds.
pub(super) fn build_all_schemas() -> Vec<(String, Value)> {
    vec![
        deployment_schema(),
        service_schema(),
        pod_schema(),
        configmap_schema(),
        secret_schema(),
        ingress_schema(),
        statefulset_schema(),
        daemonset_schema(),
        job_schema(),
        cronjob_schema(),
    ]
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/// Common metadata sub-schema (requires `name`).
fn metadata_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string", "minLength": 1 },
            "namespace": { "type": "string" },
            "labels": { "type": "object" },
            "annotations": { "type": "object" }
        }
    })
}

/// Container schema (requires `name` and `image`).
fn container_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "image"],
        "properties": {
            "name": { "type": "string", "minLength": 1 },
            "image": { "type": "string", "minLength": 1 },
            "ports": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "containerPort": { "type": "integer" }
                    }
                }
            },
            "env": { "type": "array" },
            "resources": { "type": "object" },
            "volumeMounts": { "type": "array" },
            "command": { "type": "array" },
            "args": { "type": "array" },
            "livenessProbe": { "type": "object" },
            "readinessProbe": { "type": "object" },
            "securityContext": { "type": "object" }
        }
    })
}

/// Pod spec sub-schema (requires `containers`).
fn pod_spec_schema() -> Value {
    json!({
        "type": "object",
        "required": ["containers"],
        "properties": {
            "containers": {
                "type": "array",
                "minItems": 1,
                "items": container_schema()
            },
            "initContainers": {
                "type": "array",
                "items": container_schema()
            },
            "volumes": { "type": "array" },
            "serviceAccountName": { "type": "string" },
            "restartPolicy": { "type": "string" },
            "securityContext": { "type": "object" },
            "nodeSelector": { "type": "object" },
            "tolerations": { "type": "array" },
            "affinity": { "type": "object" }
        }
    })
}

/// Pod template spec (has metadata + spec).
fn pod_template_schema() -> Value {
    json!({
        "type": "object",
        "required": ["spec"],
        "properties": {
            "metadata": { "type": "object" },
            "spec": pod_spec_schema()
        }
    })
}

/// Label selector sub-schema.
fn label_selector_schema() -> Value {
    json!({
        "type": "object",
        "required": ["matchLabels"],
        "properties": {
            "matchLabels": { "type": "object" },
            "matchExpressions": { "type": "array" }
        }
    })
}

// -------------------------------------------------------------------------
// Resource schemas
// -------------------------------------------------------------------------

fn deployment_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Deployment" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["selector", "template"],
                "properties": {
                    "replicas": { "type": "integer", "minimum": 0 },
                    "selector": label_selector_schema(),
                    "template": pod_template_schema(),
                    "strategy": { "type": "object" },
                    "minReadySeconds": { "type": "integer" }
                }
            }
        }
    });
    ("Deployment".into(), schema)
}

fn service_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Service" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["ports"],
                "properties": {
                    "ports": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["port"],
                            "properties": {
                                "port": { "type": "integer" },
                                "targetPort": {},
                                "protocol": { "type": "string" },
                                "name": { "type": "string" }
                            }
                        }
                    },
                    "selector": { "type": "object" },
                    "type": { "type": "string" },
                    "clusterIP": { "type": "string" }
                }
            }
        }
    });
    ("Service".into(), schema)
}

fn pod_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Pod" },
            "metadata": metadata_schema(),
            "spec": pod_spec_schema()
        }
    });
    ("Pod".into(), schema)
}

fn configmap_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "ConfigMap" },
            "metadata": metadata_schema(),
            "data": { "type": "object" },
            "binaryData": { "type": "object" }
        }
    });
    ("ConfigMap".into(), schema)
}

fn secret_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Secret" },
            "metadata": metadata_schema(),
            "data": { "type": "object" },
            "stringData": { "type": "object" },
            "type": { "type": "string" }
        }
    });
    ("Secret".into(), schema)
}

fn ingress_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Ingress" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "properties": {
                    "rules": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "host": { "type": "string" },
                                "http": { "type": "object" }
                            }
                        }
                    },
                    "tls": { "type": "array" },
                    "ingressClassName": { "type": "string" },
                    "defaultBackend": { "type": "object" }
                }
            }
        }
    });
    ("Ingress".into(), schema)
}

fn statefulset_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "StatefulSet" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["selector", "template", "serviceName"],
                "properties": {
                    "replicas": { "type": "integer", "minimum": 0 },
                    "selector": label_selector_schema(),
                    "template": pod_template_schema(),
                    "serviceName": { "type": "string", "minLength": 1 },
                    "volumeClaimTemplates": { "type": "array" }
                }
            }
        }
    });
    ("StatefulSet".into(), schema)
}

fn daemonset_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "DaemonSet" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["selector", "template"],
                "properties": {
                    "selector": label_selector_schema(),
                    "template": pod_template_schema(),
                    "updateStrategy": { "type": "object" }
                }
            }
        }
    });
    ("DaemonSet".into(), schema)
}

fn job_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "Job" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["template"],
                "properties": {
                    "template": pod_template_schema(),
                    "backoffLimit": { "type": "integer" },
                    "completions": { "type": "integer" },
                    "parallelism": { "type": "integer" },
                    "activeDeadlineSeconds": { "type": "integer" },
                    "ttlSecondsAfterFinished": { "type": "integer" }
                }
            }
        }
    });
    ("Job".into(), schema)
}

fn cronjob_schema() -> (String, Value) {
    let schema = json!({
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": { "type": "string" },
            "kind": { "const": "CronJob" },
            "metadata": metadata_schema(),
            "spec": {
                "type": "object",
                "required": ["schedule", "jobTemplate"],
                "properties": {
                    "schedule": { "type": "string", "minLength": 1 },
                    "jobTemplate": {
                        "type": "object",
                        "required": ["spec"],
                        "properties": {
                            "spec": {
                                "type": "object",
                                "required": ["template"],
                                "properties": {
                                    "template": pod_template_schema()
                                }
                            }
                        }
                    },
                    "concurrencyPolicy": { "type": "string" },
                    "successfulJobsHistoryLimit": { "type": "integer" },
                    "failedJobsHistoryLimit": { "type": "integer" },
                    "startingDeadlineSeconds": { "type": "integer" },
                    "suspend": { "type": "boolean" }
                }
            }
        }
    });
    ("CronJob".into(), schema)
}

#[cfg(test)]
mod tests;
