# Backup and restore

This page says what exists on main. Each part that has no code on main is marked "not available"; do not rely on it.

| Part | State | Tracking |
|---|---|---|
| Backup destination and policy schema, sinks, runner, retention (`service-backup`) | available | [service-backup](../domain/service-backup.md) |
| Scheduled backup Jobs from `service-k8s` | available | [#115](https://github.com/faberline/core/issues/115) |
| Backup and restore Job lifecycle | not available | [#114](https://github.com/faberline/core/issues/114) |
| Backup status only from a verified manifest | not available | [#116](https://github.com/faberline/core/issues/116) |
| Empty-instance restore Job handoff | not available | [#117](https://github.com/faberline/core/issues/117) |
| GKE first-adopter backup and restore run | not available | [#118](https://github.com/faberline/core/issues/118) |
| Identity-separated backup and restore Jobs with truthful status | not available | [#19](https://github.com/faberline/core/issues/19) |

## What exists

**Backup contract (`service-backup`).** The shared backup crate has the backup destination and policy schema, with the `file://`, `s3://` and `gs://` schemes. It has the sinks that store a backup: the local file sink and the GCS sink always, and the S3 sink with the `s3` feature. It has the runner that uploads one snapshot and applies retention, and the read of one exact backup object for restore or for seeding an empty volume. The read is a cold seed path, not live replica synchronization. The domain page is [service-backup](../domain/service-backup.md).

**Scheduled backup Jobs (`service-k8s`).** `service-k8s` renders backup CronJobs from the backup policy. See [service-k8s](../domain/service-k8s.md) and the 0.5.0 notes in [service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md).

## Not available

- Backup and restore Job lifecycle: not available on main; do not rely on it. Tracking [#114](https://github.com/faberline/core/issues/114).
- Backup status only from a verified manifest: not available on main; do not rely on it. Tracking [#116](https://github.com/faberline/core/issues/116).
- Empty-instance restore Job handoff: not available on main; do not rely on it. Tracking [#117](https://github.com/faberline/core/issues/117).
- GKE first-adopter backup and restore run: not available on main; do not rely on it. Tracking [#118](https://github.com/faberline/core/issues/118).
- Identity-separated backup and restore Jobs with truthful status: not available on main; do not rely on it. Tracking [#19](https://github.com/faberline/core/issues/19).
