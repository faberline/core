# service-k8s: operator helper gap

This table compares the CRD status, defaults and backup spec helpers that the
five operator crates copy (relay, defer, tape, keep, sift) with the
ServiceProfile engine in `service-k8s`. It records what the engine already
replaces, and what `service-k8s` builds in this change. Part 3 of #213.

Facts were read on `main` of each repository (core at fb0f84b; relay ad679c6,
defer bef6ba2, tape 129aa86, keep c8eb465, sift 0e51c81). Paths in the
"Copies" column are repository-relative.

| Helper | Copies (repo file:line) | Engine replaces it? | Engine file:line | Action in core | Service issue that deletes the copy |
|---|---|---|---|---|---|
| Phase rule `Ready` (desired>0 and ready>=desired) / `Reconciling` (ready>0) / `Pending` | relay `crates/relay-operator/src/reconcile.rs` L47-67; defer `crates/defer-operator/src/reconcile.rs` L163-181; keep `crates/keep-operator/src/reconcile.rs` L96-115 | no (`ProfileService::status_patch` writes only `status.readyReplicas`) | `application/operator/profile_service.rs` L186-192 | BUILD: `replica_phase` | none yet: the desk opens it in the service repository after this merges |
| Flat status object (`phase`, `observedGeneration`, `readyReplicas`, `desiredReplicas`, `message`) | relay, defer and keep files as above; tape `crates/tape-operator/src/reconcile.rs` L90-99 | no | `application/operator/profile_service.rs` L186-192 | BUILD: `flat_status_patch` | none yet: the desk opens it in the service repository after this merges |
| Message `"{r}/{d} <noun> ready"` (relay `broker pods`, defer `Defer pods`, keep `store pods`, tape `tape pods` at `reconcile.rs` L139-141) | the operator `reconcile.rs` files above (tape L139-141) | partly (the `Ready` condition message is `"{r}/{d} replicas ready"`, `domain/instance_conditions.rs` L55-58) | `domain/instance_conditions.rs` L55-58 | BUILD: `replicas_message` | none yet: the desk opens it in the service repository after this merges |
| Default storage `"10Gi"` | relay `crd.rs` L150-152; defer `crd.rs` L302-304; tape `crd.rs` L289-291; keep `crd.rs` L135-137 | no (`storage.size` has no default) | `interfaces/service_spec.rs` L29-31 | BUILD: `DEFAULT_STORAGE_SIZE` | none yet: the desk opens it in the service repository after this merges |
| Default grace seconds `10` (keep uses 30: not shared) | relay `crd.rs` L153-155; defer `crd.rs` L305-307; tape `crd.rs` L292-294 | no | `interfaces/service_spec.rs` L132-162 | BUILD: `DEFAULT_GRACE_SECS` | none yet: the desk opens it in the service repository after this merges |
| Default body limit 8 MiB (keep uses 16 MiB: not shared) | relay `crd.rs` L156-158; defer `crd.rs` L308-310 | no | `interfaces/service_spec.rs` L160-161 | BUILD: `DEFAULT_BODY_LIMIT_BYTES` | none yet: the desk opens it in the service repository after this merges |
| Default auth `"off"` | relay `crd.rs` L159-161; defer `crd.rs` L311-313 | no | `interfaces/service_spec.rs` L59-77 | BUILD: `DEFAULT_AUTH_MODE` | none yet: the desk opens it in the service repository after this merges |
| `status.conditions` (five standard types) | tape `reconcile.rs` L155-243 | yes | `domain/instance_conditions.rs` L49-135 | none | — |
| `Condition` type, `observed_conditions` | tape `crd.rs` L285-286; `reconcile.rs` L117-122 | yes | `domain/condition.rs` L17-34; `application/operator/profile_service.rs` L210-216 | none | — |
| `BackupConfigured` | tape `reconcile.rs` L155-243 | yes | `application/operator/profile_service.rs` L203 | none | — |
| `backup_run_fact` (CronJob status to `BackupSucceeded`) | tape `reconcile.rs` L297-351 | partly (the engine passes `None`, `profile_service.rs` L204) | `domain/backup_status.rs` L74-116 (Job based) | NOT BUILT: needs a decision on CronJob versus Job status | — |
| Backup wrapper `{ flatten ScheduledBackupPolicy, adminTokenSecret }` + `Deref` | relay `crd.rs` L107-127; defer `crd.rs` L270-282; tape `crd.rs` L223-244 | no | `crates/service-backup/src/domain/policy.rs` L51-60 (the policy only) | NOT BUILT: `service-k8s` does not depend on `service-backup`; a new dependency is a design decision | — |
| Placement structs, `Toleration`, `LocalObjectReference`, placement render helpers | defer `crd.rs` L180-266, `render.rs` L343-382; tape `crd/placement.rs` L11-65, `render/placement.rs` L9-45 | partly (types yes; `render_instance` does not render them, L226-227) | `interfaces/service_spec.rs` L103-118; `infrastructure/manifest/instance_render.rs` L226-227 | NOT BUILT: depends on the render work of #528 | — |
| `crd_yaml()` (unsigned-integer fix + YAML 1.1 boolean quoting) | relay `crd.rs` L169-175; defer `crd.rs` L316-322; tape `crd/schema.rs` L46-70; keep `lib.rs` L30-60 (keep has its own copy of the integer fix) | yes for the helpers | `infrastructure/crd.rs` L11, L141 | none | — |
| Capacity policy, resource limit helpers, sift CRD-to-internal conversion, `LogFormat`, `AuthMode` enums | defer, sift, tape | no, product specific | — | none (stay in the service) | — |

## Built here

- `replica_phase`, `ReplicaPhase` (`crates/service-k8s/src/domain/replica_status.rs`)
- `flat_status_patch` (`crates/service-k8s/src/domain/replica_status.rs`)
- `replicas_message` (`crates/service-k8s/src/domain/replica_status.rs`)
- `DEFAULT_STORAGE_SIZE`, `DEFAULT_GRACE_SECS`, `DEFAULT_BODY_LIMIT_BYTES`, `DEFAULT_AUTH_MODE` (`crates/service-k8s/src/domain/spec_defaults.rs`)

The service copies are not deleted by this change. The desk opens the service
issues after this merges.

## Not built here

- `backup_run_fact`: needs a decision on CronJob versus Job status.
- The backup wrapper type: `service-k8s` does not depend on `service-backup`.
- Placement types and render helpers: depend on the render work of #528.
- The rest of the table: the engine already replaces these, or they are
  product specific.
