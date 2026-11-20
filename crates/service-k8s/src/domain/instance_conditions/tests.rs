use super::*;

struct Dims(Vec<ConditionFact>);

impl DomainReadiness for Dims {
    fn dimensions(&self) -> Vec<ConditionFact> {
        self.0.clone()
    }
}

fn facts(ready: u32, desired: u32) -> InstanceFacts {
    InstanceFacts {
        ready_replicas: ready,
        desired_replicas: desired,
        storage_healthy: true,
        backup_configured: true,
        last_backup_succeeded: Some(true),
    }
}

fn find<'a>(all: &'a [ConditionFact], type_: &str) -> &'a ConditionFact {
    all.iter().find(|fact| fact.type_ == type_).unwrap()
}

#[test]
fn conditions_include_the_domain_dimensions_beside_the_five_standard_ones() {
    let shard_map = ConditionFact::new("ShardMapHealthy", ConditionStatus::True, "MapOk", "map 3");
    let all = instance_conditions(&facts(3, 3), &Dims(vec![shard_map.clone()]));
    let types: Vec<&str> = all.iter().map(|fact| fact.type_.as_str()).collect();
    assert_eq!(
        types,
        [
            "Ready",
            "Progressing",
            "StorageHealthy",
            "BackupConfigured",
            "BackupSucceeded",
            "ShardMapHealthy"
        ]
    );
    assert_eq!(
        &all[..5]
            .iter()
            .map(|f| f.type_.as_str())
            .collect::<Vec<_>>(),
        &STANDARD_CONDITION_TYPES
    );
    assert_eq!(all[5], shard_map);
}

#[test]
fn a_domain_dimension_cannot_replace_a_standard_condition() {
    let fake = ConditionFact::new("Ready", ConditionStatus::False, "AppSaysNo", "x");
    let extra = ConditionFact::new("ShardMapHealthy", ConditionStatus::True, "MapOk", "");
    let all = instance_conditions(&facts(3, 3), &Dims(vec![fake, extra.clone()]));
    assert_eq!(all.len(), 6);
    let ready = find(&all, "Ready");
    assert_eq!(ready.status, ConditionStatus::True);
    assert_eq!(ready.reason, "AllReplicasReady");
    assert_eq!(all.iter().filter(|fact| fact.type_ == "Ready").count(), 1);
    assert_eq!(all[5], extra);
}

#[test]
fn ready_and_progressing_follow_the_replica_counts() {
    use ConditionStatus::{False, True};
    // (ready, desired, Ready status, Ready reason, Progressing status, Progressing reason)
    let table = [
        (3, 3, True, "AllReplicasReady", False, "Idle"),
        (2, 3, False, "ReplicasNotReady", True, "Rolling"),
        (0, 3, False, "ReplicasNotReady", True, "Rolling"),
        (4, 3, False, "ReplicasNotReady", True, "Rolling"),
        (0, 0, False, "ReplicasNotReady", False, "Idle"),
    ];
    for (ready, desired, ready_status, ready_reason, prog_status, prog_reason) in table {
        let all = instance_conditions(&facts(ready, desired), &Dims(vec![]));
        let r = find(&all, "Ready");
        assert_eq!(
            (r.status, r.reason.as_str()),
            (ready_status, ready_reason),
            "{ready}/{desired}"
        );
        let p = find(&all, "Progressing");
        assert_eq!(
            (p.status, p.reason.as_str()),
            (prog_status, prog_reason),
            "{ready}/{desired}"
        );
    }
    let all = instance_conditions(&facts(2, 3), &Dims(vec![]));
    assert_eq!(find(&all, "Ready").message, "2/3 replicas ready");
    assert_eq!(find(&all, "Progressing").message, "2/3 replicas ready");
}

#[test]
fn backup_succeeded_is_unknown_before_the_first_backup() {
    let mut f = facts(3, 3);
    f.last_backup_succeeded = None;
    let all = instance_conditions(&f, &Dims(vec![]));
    let b = find(&all, "BackupSucceeded");
    assert_eq!(
        (b.status, b.reason.as_str()),
        (ConditionStatus::Unknown, "NoBackupYet")
    );
    f.last_backup_succeeded = Some(true);
    let b = instance_conditions(&f, &Dims(vec![]));
    assert_eq!(
        (
            find(&b, "BackupSucceeded").status,
            find(&b, "BackupSucceeded").reason.as_str()
        ),
        (ConditionStatus::True, "LastBackupSucceeded")
    );
    f.last_backup_succeeded = Some(false);
    let b = instance_conditions(&f, &Dims(vec![]));
    assert_eq!(
        (
            find(&b, "BackupSucceeded").status,
            find(&b, "BackupSucceeded").reason.as_str()
        ),
        (ConditionStatus::False, "LastBackupFailed")
    );
}

#[test]
fn storage_and_backup_configured_follow_their_flags() {
    let mut f = facts(3, 3);
    let all = instance_conditions(&f, &Dims(vec![]));
    assert_eq!(
        (
            find(&all, "StorageHealthy").status,
            find(&all, "StorageHealthy").reason.as_str()
        ),
        (ConditionStatus::True, "StorageOk")
    );
    assert_eq!(
        (
            find(&all, "BackupConfigured").status,
            find(&all, "BackupConfigured").reason.as_str()
        ),
        (ConditionStatus::True, "BackupConfigured")
    );
    f.storage_healthy = false;
    f.backup_configured = false;
    let all = instance_conditions(&f, &Dims(vec![]));
    assert_eq!(
        (
            find(&all, "StorageHealthy").status,
            find(&all, "StorageHealthy").reason.as_str()
        ),
        (ConditionStatus::False, "StorageDegraded")
    );
    assert_eq!(
        (
            find(&all, "BackupConfigured").status,
            find(&all, "BackupConfigured").reason.as_str()
        ),
        (ConditionStatus::False, "NoBackup")
    );
}

#[test]
#[cfg(feature = "controller")]
fn workload_condition_type_is_camel_case_and_ends_in_ready() {
    assert_eq!(workload_condition_type("collector"), "CollectorReady");
    assert_eq!(workload_condition_type("web-api"), "WebApiReady");
    assert_eq!(workload_condition_type("a--b"), "ABReady");
    assert_eq!(workload_condition_type("x9"), "X9Ready");
}

#[test]
#[cfg(feature = "controller")]
fn a_workload_with_a_ready_replica_is_true() {
    assert_eq!(
        workload_condition("collector", 1),
        ConditionFact::new(
            "CollectorReady",
            ConditionStatus::True,
            "WorkloadReady",
            "collector: 1 replicas ready"
        )
    );
}

#[test]
#[cfg(feature = "controller")]
fn a_workload_with_no_ready_replica_is_false() {
    assert_eq!(
        workload_condition("collector", 0),
        ConditionFact::new(
            "CollectorReady",
            ConditionStatus::False,
            "WorkloadNotReady",
            "collector: 0 replicas ready"
        )
    );
}

#[test]
#[cfg(feature = "controller")]
fn a_workload_condition_type_never_equals_a_standard_type() {
    for name in ["a", "ready", "backup-succeeded", "x"] {
        assert!(!STANDARD_CONDITION_TYPES.contains(&workload_condition_type(name).as_str()));
    }
}
