use super::*;
use crate::infrastructure::schema_validation::registry::SchemaRegistry;
use jsonschema::Validator;

#[test]
fn test_all_k8s_schemas_compile() {
    let schemas = build_all_schemas();
    assert_eq!(schemas.len(), 10);
    for (kind, schema) in &schemas {
        Validator::new(schema)
            .unwrap_or_else(|e| panic!("Schema for {} failed to compile: {}", kind, e));
    }
}

#[test]
fn test_deployment_valid() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": "web" },
        "spec": {
            "selector": { "matchLabels": { "app": "web" } },
            "template": {
                "spec": {
                    "containers": [{
                        "name": "web",
                        "image": "nginx:1.25"
                    }]
                }
            }
        }
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(diags.is_empty(), "expected no errors: {:?}", diags);
}

#[test]
fn test_deployment_missing_selector() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": "web" },
        "spec": {
            "template": {
                "spec": {
                    "containers": [{ "name": "c", "image": "img:1" }]
                }
            }
        }
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(!diags.is_empty(), "should report missing selector");
    assert!(diags.iter().any(|d| d.message.contains("selector")));
}

#[test]
fn test_service_missing_ports() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": { "name": "svc" },
        "spec": {}
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(!diags.is_empty(), "should report missing ports");
}

#[test]
fn test_pod_missing_containers() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "p" },
        "spec": {}
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(!diags.is_empty(), "should report missing containers");
}

#[test]
fn test_cronjob_missing_schedule() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "batch/v1",
        "kind": "CronJob",
        "metadata": { "name": "cj" },
        "spec": {
            "jobTemplate": {
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [{ "name": "c", "image": "i:1" }]
                        }
                    }
                }
            }
        }
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(!diags.is_empty(), "should report missing schedule");
    assert!(diags.iter().any(|d| d.message.contains("schedule")));
}

#[test]
fn test_unknown_kind_no_errors() {
    let registry = SchemaRegistry::new("1.30");
    let doc = json!({
        "apiVersion": "custom.io/v1",
        "kind": "MyCustomResource",
        "metadata": { "name": "x" },
        "spec": {}
    });
    let diags = registry.validate_k8s(&doc, "1.30");
    assert!(diags.is_empty(), "unknown kind should produce no errors");
}
