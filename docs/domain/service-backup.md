# service-backup

service-backup is the shared backup contract for services. The service owns
snapshot consistency, the snapshot bytes, restore semantics and the endpoint
that produces the snapshot. service-backup owns what surrounds those bytes: the
destination and policy schema, the sink port and its sinks, the runner that
uploads a snapshot and applies retention, the read of one exact object for
restore or bootstrap, and (with the `http-client` feature) the admin-snapshot
transport. No other core crate depends on it; eight downstream repos do: beam,
defer, keep, loom, lumen, relay, sift and tape.

**Form:** layered · **Depends on:** cli-std, service-auth (only with the `http-client` feature), storage-durable, storage-object · **Crate:** [`crates/service-backup`](../../crates/service-backup)

## Model

- **Backup destination** — `BackupDestination`: `Local` (a path and an
  optional prefix), `S3` (a bucket and a prefix, plus an optional region,
  endpoint and credentials secret) or `Gcs` (a bucket and a prefix, plus an
  optional credentials secret), serialized as an enum tagged by `type`. A
  destination names a prefix, not an object.
- **Scheme table** — `SUPPORTED_SCHEMES`: the `SchemeInfo` for each scheme
  `from_uri` accepts (`file://`, `s3://`, `gs://`), in parse order, with a
  description and whether this build links a sink for it.
- **BackupPolicy** — the runtime policy: a cron `schedule`, a destination and a
  retention policy. The operator turns the schedule into a CronJob schedule.
- **ScheduledBackupPolicy** — the flat form a custom resource embeds:
  `schedule`, a destination URI and an optional `retentionSecs`. It exists
  because the tagged destination enum cannot be embedded in a Kubernetes
  structural schema.
- **RetentionPolicy** (service-backup) — `max_age_seconds`: objects older than
  this are pruned after a successful put. `None` turns pruning off.
- **Backup sink** — `LocalFsSink`, `GcsSink`, an S3 sink (with the `s3`
  feature), or `UnsupportedCloudSink` for S3 in a build without it.
  `sink_from_destination` is the only place a destination becomes a sink;
  it is wiring and lives in the `src/app` composition root.
- **Backup run** — `run_backup_once` puts one payload and applies retention.
  Its `BackupRunResult` holds the `BackupObject` written (sink identity, key,
  byte count, Unix seconds) and the number of objects pruned.
- **Backup object** — the one exact object `fetch_backup_object` reads from a
  `file://`, `s3://` or `gs://` URI for restore or empty-volume bootstrap: a
  cold seed path, not live replica synchronization.
- **admin snapshot** — the body a service returns from `GET /admin/backup` and
  accepts on `POST /admin/restore`. `AdminSnapshotTransport` moves it within an
  `AdminSnapshotTransportConfig` (`Default` plus `with_*` builders for the
  connect, operation and idle timeouts and the diagnostic size, read through
  getters); an `AdminSnapshotRequest` adds a static or
  projected bearer and extra headers; an `AdminSnapshotDiagnostic` describes a
  failed request.

## Ports

- **`BackupSink`** (domain) — `put` stores a payload at a timestamp and
  returns its key; `prune` removes objects older than a maximum age and
  returns how many; both fail with `BackupSinkError`. `identity` names the
  sink. It is `Send + Sync + 'static`. Only the infrastructure sinks above
  implement it; no downstream repo does.
- **Snapshot source** (domain, crate-internal, `http-client` feature) —
  fetches the exact admin snapshot bytes for `run_admin_snapshot_backup`,
  failing with its own `SnapshotSourceError`. Its adapter wraps
  `AdminSnapshotTransport::fetch_exact`.
- **Bearer token source** (domain, crate-internal, `http-client` feature) —
  the admin bearer the transport reads before every request. The adapter over
  service-auth's projected token file lives in `src/app`, behind
  `AdminSnapshotRequest::with_projected_bearer`; only the composition root
  names service-auth.

## Invariants

- `from_uri` trims its input and returns a `DestinationError` for an empty
  URI, a `file://` URI without a path, and an `s3://` or `gs://` URI without a
  bucket; for any other scheme the error lists `SUPPORTED_SCHEMES`. A URI
  never sets a region, an endpoint or a credentials secret.
- An `s3://` destination parses in every build: S3 support is a question about
  the sink, not the URI. Without the `s3` feature, `put` and `prune` fail with a
  message naming `--features s3`.
- `ScheduledBackupPolicy::to_runtime_policy` (also reached through `TryFrom`)
  is the only validated conversion. It returns a `PolicyError` for a blank
  schedule and for any destination `from_uri` rejects, whose message it passes
  on unchanged, but does not parse the cron expression, so an invalid cron
  fails in Kubernetes instead. A missing `retentionSecs` keeps every object.
- `run_backup_once` prunes only after a successful put, and only when a
  maximum age is set. The caller passes the timestamp, and the payload must
  already be a consistent snapshot.
- Keys and pruning per sink:
  - Local: `<prefix>-<unix seconds>.json`, written atomically with fsync. Prune
    removes every file in the directory older than the cutoff by modification
    time, so each sink needs its own directory.
  - GCS: `<prefix>-<unix seconds>.json`; prune compares the updated time of
    each object under the prefix.
  - S3: `[<prefix>/]backup-<unix seconds>.json`; prune touches only keys of
    that shape and reads their age from the key.
  - The GCS and S3 sinks refuse a credentials secret and use ambient
    credentials (Workload Identity or ADC; the AWS environment).
- The strict admin-snapshot transport never follows a redirect and never
  retries. Fetch accepts only 200 and restore only 204. Each chunk read is
  bounded by the idle timeout and the whole call by the operation timeout
  (defaults: 10 s connect, 2 h operation, 30 s idle, 8 KiB diagnostic). A
  fetch with an unexpected status keeps at most the config's
  `max_diagnostic_bytes()` of the body; `fetch_exact` and `restore_exact` return redacted errors that hold
  neither the body nor the token. A projected bearer token is re-read from its
  file before every request, so a kubelet rotation takes effect.
- `run_admin_snapshot_backup` fetches through that strict transport and opens
  the sink only after a successful fetch.

## Published language

No other core context depends on service-backup. Downstream repos import from
the crate root, where every public name is re-exported: `run_backup_once` and
`sink_from_destination` (beam, keep, loom, lumen, sift), `fetch_backup_object`
(defer, lumen, sift, tape), the strict transport (lumen, sift), `GcsSink`
(sift) and `SUPPORTED_SCHEMES` (tape). The one public module path is
`service_backup::llm` (lumen, tape): the llm topic (v1) `TOPIC` and its
sectioned form `SECTIONED_TOPICS`, whose destination section is rendered from
`SUPPORTED_SCHEMES` at call time. `llm` keeps its path (`src/api/`) because
the root does not re-export its names.

## Exceptions and debts

- **Checker exceptions:**
  - B2 (`schemars`), long-term: `BackupDestination`, `ScheduledBackupPolicy`,
    `BackupPolicy` and `RetentionPolicy` derive `schemars::JsonSchema`
    because downstream CRD specs embed them; a separate interfaces copy
    would duplicate the wire contract.
  - P2 removed the others: the sink and snapshot source ports moved to the
    domain (B3 `application->infrastructure`), the `llm` topic lists schemes
    through an application query (B3 `interfaces->domain`), and the
    transport reads its bearer through a domain port whose service-auth
    adapter lives in `src/app` (B4).
- **Debts:**
  - Public fields built with struct literals (ADR D2):
    `ScheduledBackupPolicy` in lumen and relay tests and in tape's
    end-to-end tests; `RetentionPolicy` in loom. lumen's unit tests build
    `BackupDestination::Local` values, and sift destructures every field
    of the `Gcs` variant (enum variant fields are always public).
  - `LocalFsSink` has public `root` and `prefix` fields.
