# Limits

Sizes and rates of the operator crates, one table per crate. Columns: `Limit`,
`Value`, `Proof`, `Source`. A `Proof` is a test (its file and function name) that
asserts the value, or the words `not measured` when no such test exists. A row
that says "not measured" has no test or measurement yet.

## service-http

| Limit | Value | Proof | Source |
|---|---|---|---|
| admission refill window | 60 s (`DEFAULT_REFILL_SECS`) | not measured | `crates/service-http/src/application/admission_config.rs` |
| admission maximum tracked keys | 1024 (`DEFAULT_MAX_KEYS`) | not measured | `crates/service-http/src/application/admission_config.rs` |
| `body_limit_bytes` | set by the service binary | not measured | `crates/service-http/src/infrastructure/config.rs` |
| `grace_secs` | set by the service binary | not measured | `crates/service-http/src/infrastructure/config.rs` |

## service-k8s

| Limit | Value | Proof | Source |
|---|---|---|---|
| probe period | 10 s | not measured | `crates/service-k8s/src/domain/lifecycle.rs` |
| probe timeout | 1 s | not measured | `crates/service-k8s/src/domain/lifecycle.rs` |
| probe failure threshold | 3 | not measured | `crates/service-k8s/src/domain/lifecycle.rs` |
| probe success threshold | 1 | not measured | `crates/service-k8s/src/domain/lifecycle.rs` |
| metrics address | `0.0.0.0:9090` | `crates/service-k8s/src/interfaces/metrics.rs`, test `an_unparseable_address_override_falls_back_to_the_default` (asserts `DEFAULT_METRICS_ADDR`) | `crates/service-k8s/src/interfaces/metrics.rs` |

The four probe values are the `ProbeTiming::default()` values. No test asserts
them, so their rows say "not measured".

## service-observability

| Limit | Value | Proof | Source |
|---|---|---|---|
| none | none | not measured | `crates/service-observability/src/config.rs` |

## service-backup

| Limit | Value | Proof | Source |
|---|---|---|---|
| `CONNECT_TIMEOUT` | 10 s | not measured | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `OPERATION_TIMEOUT` | 2 h | not measured | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `BACKUP_IDLE_TIMEOUT` | 30 s | not measured | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `MAX_RESTORE_LEDGER_BYTES` | 1 MiB (1024 * 1024 bytes) | `crates/service-backup/tests/it/restore_ledger.rs`, test `rejected_json_documents` (asserts `OversizedJson` for `MAX_RESTORE_LEDGER_BYTES + 1` bytes) | `crates/service-backup/src/domain/restore_ledger.rs` |
| `MAX_RESTORE_LEDGER_ARTIFACTS` | 1024 | `crates/service-backup/tests/it/restore_ledger.rs`, test `each_invalid_begin_input` (asserts `TooManyArtifacts` for `MAX_RESTORE_LEDGER_ARTIFACTS + 1` artifacts) | `crates/service-backup/src/domain/restore_ledger.rs` |
| `MAX_SET_ID_BYTES` | 128 | `crates/service-backup/tests/it/restore_ledger.rs`, test `each_invalid_begin_input` (asserts `InvalidSetId` for a 129-byte set id) | `crates/service-backup/src/domain/restore_ledger.rs` |
| `MAX_MANIFEST_BYTES` | 1 MiB (1 048 576 bytes) | `crates/service-backup/tests/it/backup_set.rs`, test `oversized_manifest` (asserts `ManifestTooLarge` with `max == MAX_MANIFEST_BYTES` for `MAX_MANIFEST_BYTES + 1` bytes) | `crates/service-backup/src/domain/backup_set.rs` |
| `MAX_ARTIFACTS` | 1024 | not measured | `crates/service-backup/src/domain/backup_set.rs` |
| `MAX_ARTIFACT_NAME_BYTES` | 256 | not measured | `crates/service-backup/src/domain/backup_set.rs` |

The transport limits are the defaults of `AdminSnapshotTransportConfig`. The
tests that call `with_*` builders set their own values, so they do not prove
the defaults.

A row that says "not measured" has no test or measurement yet.
