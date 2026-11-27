//! Consumer-side checks for issue #656: a service CRD on kube 5.2 embeds
//! `ScheduledBackupPolicy`, so every backup type must derive `JsonSchema`
//! from schemars 1, and the workspace must hold one schemars major.

use kube::{CustomResource, CustomResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use service_backup::{
    BackupDestination, BackupObject, BackupPolicy, BackupRunResult, RetentionPolicy,
    ScheduledBackupPolicy,
};

/// A CRD spec that flattens `ScheduledBackupPolicy`. Before #656 this did not
/// compile: `schemars` 0.8 `JsonSchema` does not satisfy the kube 5.2 derive.
#[derive(CustomResource, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "example.faberline.dev",
    version = "v1",
    kind = "Probe",
    namespaced
)]
struct ProbeSpec {
    #[serde(flatten)]
    backup: ScheduledBackupPolicy,
}

#[test]
fn scheduled_backup_policy_embeds_in_a_kube_crd_spec() {
    let crd = Probe::crd();
    let value = serde_json::to_value(&crd).unwrap();
    let spec_properties = &value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"];
    let keys: Vec<&str> = spec_properties
        .as_object()
        .expect("spec properties in the CRD schema")
        .keys()
        .map(String::as_str)
        .collect();
    assert!(keys.contains(&"destination"), "keys: {keys:?}");
    assert!(keys.contains(&"retentionSecs"), "keys: {keys:?}");
    assert!(keys.contains(&"schedule"), "keys: {keys:?}");
}

#[test]
fn every_service_backup_schema_type_is_on_schemars_1() {
    let types = [
        serde_json::to_value(schemars::schema_for!(BackupObject)).unwrap(),
        serde_json::to_value(schemars::schema_for!(BackupRunResult)).unwrap(),
        serde_json::to_value(schemars::schema_for!(BackupDestination)).unwrap(),
        serde_json::to_value(schemars::schema_for!(BackupPolicy)).unwrap(),
        serde_json::to_value(schemars::schema_for!(RetentionPolicy)).unwrap(),
        serde_json::to_value(schemars::schema_for!(ScheduledBackupPolicy)).unwrap(),
    ];
    for schema in &types {
        assert_eq!(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "schema is not schemars 1: {schema}"
        );
    }
}

#[test]
fn lock_has_one_schemars_major() {
    let lock =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    let mut lines = lock.lines();
    let mut versions = Vec::new();
    while let Some(line) = lines.next() {
        if line == r#"name = "schemars""# {
            let version = lines.next().unwrap();
            versions.push(version.to_string());
        }
    }
    assert!(!versions.is_empty(), "no schemars package in Cargo.lock");
    for version in &versions {
        assert!(
            version.starts_with(r#"version = "1."#),
            "schemars in Cargo.lock is not major 1: {version}"
        );
    }
}

#[test]
fn no_crate_manifest_names_schemars_0_8() {
    let crates_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../..", "/crates");
    for entry in std::fs::read_dir(crates_dir).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            assert!(
                !text.contains(r#"schemars = "0.8""#),
                "{} still names schemars 0.8",
                manifest.display()
            );
        }
    }
}
