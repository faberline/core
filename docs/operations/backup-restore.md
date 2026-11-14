# Backup and restore

This page says what backup and restore do on `main` today. Each part that has
no code on `main` is marked "not available". Do not rely on a part marked that
way: no procedure for it is documented here.

| Part | State | Tracking |
|---|---|---|
| Backup destination and policy schema, sinks, runner, retention (`service-backup`) | `available` | [service-backup](../domain/service-backup.md) |
| Scheduled backup Jobs from `service-k8s` | `available` | [#115](https://github.com/faberline/core/issues/115) |
| Backup and restore Job lifecycle | `not available` | [#114](https://github.com/faberline/core/issues/114) |
| Backup status only from a verified manifest | `not available` | [#116](https://github.com/faberline/core/issues/116) |
| Empty-instance restore Job handoff | `not available` | [#117](https://github.com/faberline/core/issues/117) |
| GKE first-adopter backup and restore run | `not available` | [#118](https://github.com/faberline/core/issues/118) |
| Identity-separated backup and restore Jobs with truthful status | `not available` | [#19](https://github.com/faberline/core/issues/19) |

## What exists

The shared contract crate `service-backup` defines the backup destination and
policy schema, the sink port with its sinks, the runner that uploads one
snapshot and applies retention, and the read of one exact backup object for
restore or empty-volume bootstrap. That read is a cold seed path, not live
replica synchronization. No other core crate depends on `service-backup`. See
[service-backup](../domain/service-backup.md).

The 0.5.0 release notes list the backup CronJob and rollout planners of
`service-k8s`, with `BackupSetV1` and `RestoreLedger`. See
[release notes 0.5.0 for service-k8s and service-backup](release-notes-0.5.0/service-k8s-backup.md).

## Not available

- Backup and restore Job lifecycle: not available on main; do not rely on it.
- Backup status only from a verified manifest: not available on main; do not rely on it.
- Empty-instance restore Job handoff: not available on main; do not rely on it.
- GKE first-adopter backup and restore run: not available on main; do not rely on it.
- Identity-separated backup and restore Jobs with truthful status: not available on main; do not rely on it.
