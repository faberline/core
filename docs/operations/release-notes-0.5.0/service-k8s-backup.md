# Release notes 0.5.0: service-k8s and service-backup

Range checked: `v0.4.14` (`029e765e`) to `origin/main` (`627d8121`).

## Crates checked

- `crates/service-k8s`
- `crates/service-backup`

## Breaking changes

### service-k8s: stateful render helpers return `Result`

Before: `service_statefulset(p) -> Value`, `sharded_statefulset(p) -> Value` and `render_compat_service_statefulset(p) -> Value`.
After: each returns `Result<Value, StatefulInstanceError>`. The helpers are in `service_k8s::render`.

Migration: add `?` where the value is used, or match on `StatefulInstanceError`.

### service-k8s: `RenderCtx` fields are private

Before: `RenderCtx { pub app, pub manager, pub api_version, pub kind, pub name, pub ns, pub owner }` as a struct literal.
After: `RenderCtx::new(app, manager, api_version, kind, name, ns)`. Read with `app()`, `manager()`, `api_version()`, `kind()`, `name()`, `ns()` and `owner() -> Option<&Value>`. The constructor takes no owner argument.
Commit: `ec3b9e79`.

Migration: replace the struct literal with `RenderCtx::new(..)` and each field read with its accessor.

### service-k8s: operator contract types have private fields

Before: `ReadinessTarget.kind`, `ReadyFacts.ready`, `ReconcilePlan.children` and `.context`, `PruneTarget.api_version` and `.kind`, and `ClusterScopedChild.api_version`, `.kind`, `.expected_labels` and `.desired` were public.
After: each type has a constructor and accessors:
- `ReadinessTarget::new(kind, name)`, with `kind()` and `name()`.
- `ReadyFacts::new(ready)`, with `ready()`.
- `ReconcilePlan::new(children, context)`, with `children()` and `context()`.
- `PruneTarget::new(api_version, kind, name)`, with `api_version()`, `kind()` and `name()`.
- `ClusterScopedChild::new(api_version, kind, name, desired)`, with `api_version()`, `kind()`, `name()`, `expected_labels()` and `desired()`. `expected_labels` starts empty.
Commit: `0b655184`.

Migration: replace struct literals with the constructor and field reads with the accessors.

### service-k8s: workload and RBAC plan fields are private

Before: `RbacRulePlan` (`api_groups`, `resources`, `resource_names`, `verbs`), `RolePlan.rules`, `RoleBindingPlan` (`role_name`, `namespace`, `subjects`), and `CronJobPlan` (`name`, `schedule`, `pod`, `successful_jobs_history_limit`, `failed_jobs_history_limit`) were public.
After: `RbacRulePlan::new(api_groups, resources, verbs)`; `resource_names` starts empty and has no argument. Each plan has accessors, for example `rules()`, `role_name()`, `namespace()`, `subjects()`, `schedule()`, `pod()` and `failed_jobs_history_limit()`. `CronJobPlan::new(name, schedule, pod)` builds a CronJob plan.
Commit: `1b5b257c`.

Migration: replace struct literals with the constructors and field reads with the accessors.

### service-k8s: helpers removed

Removed, with no item of the same name in this release:
- `PROJECTED_KEYS` (constant).
- `EphemeralIssuer::anchor_pem`.
- `WorkloadIdentityTokenSource::with_sts_endpoint`.
- `ConditionStatus::from_bool`.
- `ShardSplitPlan::requires_split`.
- `ReplicaLayerPlan::requires_membership_change`.
- `NetworkPeerPlan::with_except`.
- `NetworkPeerPlan::same_namespace`.
- The type alias `KubernetesStoreError`.
Commit: `662093b7`.

Migration: `KubernetesStoreError` becomes `service_k8s::certificate::StoreError`. `NetworkPeerPlan::same_namespace()` becomes `NetworkPeerPlan::same_namespace_pods(selector)`, which takes a label selector. The other items have no replacement in this release; the app computes the value itself.

### service-backup: `BackupDestination::from_uri` returns `DestinationError`

Before: `from_uri(raw) -> anyhow::Result<Self>`.
After: `from_uri(raw) -> Result<Self, DestinationError>`.
Commit: `1326e165`.

Migration: callers that use `?` into `anyhow` need no change. Code that matches on the error must match `DestinationError`.

### service-backup: `ScheduledBackupPolicy::to_runtime_policy` returns `PolicyError`

Before: `to_runtime_policy(&self) -> anyhow::Result<BackupPolicy>`.
After: `to_runtime_policy(&self) -> Result<BackupPolicy, PolicyError>`.
Commit: `1326e165`.

Migration: callers that use `?` into `anyhow` need no change. Code that matches on the error must match `PolicyError`.

### service-backup: `BackupSink` methods return `BackupSinkError`

Before: `put(...) -> anyhow::Result<String>` and `prune(...) -> anyhow::Result<usize>`.
After: `put(...) -> Result<String, BackupSinkError>` and `prune(...) -> Result<usize, BackupSinkError>`. `BackupSink` moved to the domain layer, but `service_backup::BackupSink` is still exported at the root. `BackupSinkError::other(err)` builds the error from any `Error`.
Commit: `7c55f011`.

Migration: a sink implementation returns `BackupSinkError::other(err)` for its own failures. Callers that use `?` into `anyhow` need no change.

### service-backup: `fetch_admin_snapshot` is removed

Before: `fetch_admin_snapshot(base_url, token: Option<&str>) -> anyhow::Result<Vec<u8>>`, under the `http-client` feature.
After: removed. `AdminSnapshotTransport` and `AdminSnapshotRequest` are the replacement, and both are still exported under `http-client`.
Commit: `7c55f011`.

Migration:

```rust
let transport = AdminSnapshotTransport::new()?;
let request = match token {
    Some(t) => AdminSnapshotRequest::new().with_static_bearer(t),
    None => AdminSnapshotRequest::new(),
};
let bytes = transport.fetch(base_url, &request).await?;
```

The error type is `AdminSnapshotRequestError`. `fetch` applies the transport's timeouts and its diagnostic size limit, which the removed function did not.

### service-backup: `BackupDestination::default_prefix` is removed

Before: `default_prefix(&self) -> String`, which returned the configured prefix, or `backup` when none was set.
After: removed, with no replacement method.
Commit: `060c8bd4`.

Migration: the app computes the prefix itself (the configured prefix, or `backup` when none was set). Pass it to `SetKeyLayout::new(prefix)` for set keys.

### service-backup: `AdminSnapshotTransportConfig` fields are private

Before: `pub connect_timeout`, `pub operation_timeout`, `pub response_idle_timeout` and `pub max_diagnostic_bytes`.
After: builders `with_connect_timeout`, `with_operation_timeout`, `with_response_idle_timeout` and `with_max_diagnostic_bytes`, and accessors of the same names without `with_`. `AdminSnapshotTransportConfig::default()` is unchanged.
Commit: `c542700a`.

Migration: `cfg.operation_timeout = d` becomes `cfg = cfg.with_operation_timeout(d)`. A struct literal becomes `AdminSnapshotTransportConfig::default().with_...(..)`.

## Fixes apps must take

None. The only fix in these crates, `25067b3c` (`AdminSnapshotRequest` is `UnwindSafe` and `RefUnwindSafe` again), restores what v0.4.14 already had. An app has nothing to change.

## New

- service-k8s: `plan_backup_job`, `backup_job_name`, `BackupJobPolicy`, `ObservedBackupJob`, `BackupJobDecision`, `BackupJobPhase` and `BackupJobError`, which plan a backup CronJob.
- service-k8s: `plan_rollout`, `RolloutGate`, `RolloutStep`, `ObservedRollout`, `ObservedMember`, `Ordinal` and `HoldReason`, in the new `rollout` module, which plan one StatefulSet member update at a time.
- service-k8s: `KeyAndCsrGenerator` and `RcgenCsrGenerator`, the CSR port and its implementation.
- service-k8s: `LeafParser` and `X509LeafParser`, the leaf-parsing port and its implementation.
- service-backup: `BackupSetV1`, the versioned backup set record, with `BackupSetError`. (The spec named this item `BackupSet`; the exported name is `BackupSetV1`.)
- service-backup: `RestoreLedger`, the record of a restore's progress, with `RestoreLedgerError`, `RestorePhase` and `ResumePoint`.
- service-backup: `SetKeyLayout` and `SetKeyError`, the object keys for a backup set.
- service-backup: `ArtifactVerifier`, `ArtifactEntry` and `ArtifactStage`, which check the artifacts in a set.
- service-backup: `plan_retention`, `RetentionPlan`, `SetInventory` and `SetState`, the retention planner.
- service-backup: `resume_or_begin`, which chooses whether to resume an interrupted backup.
- service-backup: `SnapshotSource` and `SnapshotSourceError`, the snapshot source port.
- service-backup: `BearerTokenSource` and `BearerTokenError`, the bearer token port.
- service-backup: `DestinationError`, `PolicyError` and `BackupSinkError`, the typed errors of the destination, policy and sink.
- service-backup: `MAX_ARTIFACTS`, `MAX_ARTIFACT_NAME_BYTES`, `MAX_MANIFEST_BYTES`, `MAX_RESTORE_LEDGER_ARTIFACTS`, `MAX_RESTORE_LEDGER_BYTES` and `MAX_SET_ID_BYTES`, the limits on backup sets and ledgers.
