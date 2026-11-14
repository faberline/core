# Configuration reference

Every environment variable, flag and config key that the operator crates read
outside their tests. Each row gives the name, the default, the meaning and the
source file. A default of "set by the binary" means that the library takes the
value from its caller and has no default of its own.

## service-k8s

| Name | Default | Meaning | Source |
|---|---|---|---|
| `OPERATOR_METRICS_ADDR` | `0.0.0.0:9090` | Bind address of the scrape listener. An unparseable value logs a warning and falls back to the default. | `crates/service-k8s/src/interfaces/metrics.rs` |
| `POD_NAME` | unset: falls back to `HOSTNAME`, then the manager name | Leader-election identity of this replica. | `crates/service-k8s/src/app/operator.rs` |
| `HOSTNAME` | unset: falls back to the manager name | Leader-election identity, used when `POD_NAME` is unset. | `crates/service-k8s/src/app/operator.rs` |
| `POD_NAMESPACE` | `<manager>-system` | Namespace of the leader-election Lease. | `crates/service-k8s/src/app/operator.rs` |

## service-observability

| Name | Default | Meaning | Source |
|---|---|---|---|
| `RUST_LOG` | the configured log level | Filter for the log output. When set, it wins over the configured level. | `crates/service-observability/src/logging.rs` |

The other settings of this crate are typed values that the caller passes in:
`log_level` (`trace`, `debug`, `info`, `warn`, `error`), `log_format` (`Pretty`
or `Json`, type `LogFormat`) and `otlp_endpoint` (optional; `None` disables
OTLP). There is no default in the crate. See
`crates/service-observability/src/config.rs`.

## service-http

The binary of each service maps its own `SERVICE_*` prefix to these fields with
clap `env =`. The crate itself reads no environment variable for them.

| Name | Default | Meaning | Source |
|---|---|---|---|
| `host` | set by the binary; `0.0.0.0` on k8s, `127.0.0.1` for local development | Bind host. | `crates/service-http/src/infrastructure/config.rs` |
| `port` | set by the binary | Bind port. | `crates/service-http/src/infrastructure/config.rs` |
| `log_level` | set by the binary | Base log level. `RUST_LOG` still wins. | `crates/service-http/src/infrastructure/config.rs` |
| `log_format` | set by the binary | Log output format (`LogFormat`). | `crates/service-http/src/infrastructure/config.rs` |
| `grace_secs` | set by the binary | Graceful-drain window after SIGTERM, before the listener closes. | `crates/service-http/src/infrastructure/config.rs` |
| `body_limit_bytes` | set by the binary | Maximum request body of the data plane. The probe routes have no body limit. | `crates/service-http/src/infrastructure/config.rs` |
| `otlp_endpoint` | unset (`None`) | OTLP gRPC endpoint for trace export. Unset disables OTLP. | `crates/service-http/src/infrastructure/config.rs` |

Admission control reads these keys through `AdmissionConfig::from_env(prefix)`.
Each service binary maps them to its own prefix, for example
`SERVICE_ADMISSION_READ_CAPACITY`.

| Name | Default | Meaning | Source |
|---|---|---|---|
| `<PREFIX>_ADMISSION_READ_CAPACITY` | unset (class off) | Capacity of the read class. A positive integer. | `crates/service-http/src/application/admission_config.rs` |
| `<PREFIX>_ADMISSION_WRITE_CAPACITY` | unset (class off) | Capacity of the write class. A positive integer. | `crates/service-http/src/application/admission_config.rs` |
| `<PREFIX>_ADMISSION_ADMIN_CAPACITY` | unset (class off) | Capacity of the admin class. A positive integer. | `crates/service-http/src/application/admission_config.rs` |
| `<PREFIX>_ADMISSION_REFILL_SECS` | `60` | Refill window of every class. Set only with a capacity; otherwise startup fails with `OrphanedCommonSetting`. | `crates/service-http/src/application/admission_config.rs` |
| `<PREFIX>_ADMISSION_MAX_KEYS` | `1024` | Maximum number of tracked keys. Set only with a capacity; otherwise startup fails with `OrphanedCommonSetting`. | `crates/service-http/src/application/admission_config.rs` |

## service-backup

The library reads no environment variable outside its tests. Its only
environment reads are in the live S3 integration test, which is skipped when
`SERVICE_BACKUP_S3_TEST_BUCKET` is unset.

| Name | Default | Meaning | Source |
|---|---|---|---|
| `SERVICE_BACKUP_S3_TEST_BUCKET` | unset: the test is skipped | Bucket of the live S3 test. | `crates/service-backup/src/infrastructure/s3.rs` |
| `SERVICE_BACKUP_S3_TEST_REGION` | the value of `DEFAULT_S3_REGION` in the same file | Region of the live S3 test. | `crates/service-backup/src/infrastructure/s3.rs` |
| `SERVICE_BACKUP_S3_TEST_ENDPOINT` | unset | Endpoint of the live S3 test, for an S3-compatible service. | `crates/service-backup/src/infrastructure/s3.rs` |
| `SERVICE_BACKUP_S3_TEST_PREFIX` | `service-backup-tests` | Key prefix of the live S3 test. | `crates/service-backup/src/infrastructure/s3.rs` |

The public transport configuration is a Rust value, not an environment
variable. Its defaults come from `Default`:

| Type | Field | Default | Source |
|---|---|---|---|
| `AdminSnapshotTransportConfig` | `connect_timeout` | 10 s | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `AdminSnapshotTransportConfig` | `operation_timeout` | 2 h | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `AdminSnapshotTransportConfig` | `response_idle_timeout` | 30 s | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `AdminSnapshotTransportConfig` | `max_diagnostic_bytes` | 8 KiB | `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |

## cli-std

| Name | Default | Meaning | Source |
|---|---|---|---|
| `GH_TOKEN` | unset | GitHub token. The first non-empty value (after trimming) wins. Feature `online`. | `crates/cli-std/src/infrastructure/github.rs` |
| `GITHUB_TOKEN` | unset | GitHub token, used when `GH_TOKEN` is unset or blank. Then `gh auth token` is tried. Feature `online`. | `crates/cli-std/src/infrastructure/github.rs` |
| `AXIOM_COURIER_URL` | unset | URL of the courier proxy. Trimmed; a blank value counts as unset. Feature `online`. | `crates/cli-std/src/infrastructure/courier.rs` |
| `AXIOM_COURIER_TOKEN` | unset | Courier client credential, sent as a bearer token. Trimmed; a blank value counts as unset. Feature `online`. | `crates/cli-std/src/infrastructure/courier.rs` |
| `--context` | unset: kubectl's current context | Kubernetes context passed to kubectl by `connect`. Feature `k8s`. | `crates/cli-std/src/infrastructure/connect.rs` |

The crate registers no verb of its own. The registry in
`crates/cli-std/src/api/registry.rs` only re-exports `CliModule` and
`CLI_MODULES`; each crate that adds a verb registers it there, so the verb list
belongs to that crate.
