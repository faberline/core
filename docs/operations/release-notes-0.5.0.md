# Release notes 0.5.0

This page lists the changes from `v0.4.14` to `0.5.0`. The details are in the six group files in [release-notes-0.5.0/](release-notes-0.5.0/). For migration help, see [docs/migration](../migration/).

## Breaking changes

- raft-core: `NodeId`, `Term` and `Index` are newtypes, not `u64` aliases ([raft](release-notes-0.5.0/raft.md))
- raft-core: `Membership` fields are private ([raft](release-notes-0.5.0/raft.md))
- raft-core: `RaftTransport` trait removed ([raft](release-notes-0.5.0/raft.md))
- raft-core: `RaftNode::learner_replication_gap` removed ([raft](release-notes-0.5.0/raft.md))
- raft-core: `AppendReq` and `AppendResp` have a new field `read_round` ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `cluster` and `group` modules removed ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `ensure_static_membership_unchanged` removed ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `RaftStore::cache_footprint` removed ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `GroupId` field is private ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `AssignmentEpoch` is a newtype, not a `u64` alias ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `ClusterDims` fields are private ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `HostConfig` fields are private ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: state-machine and membership port errors are typed ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: `RaftStatus` has a new field `peer_match_index` ([raft](release-notes-0.5.0/raft.md))
- raft-runtime: a new leader appends a no-op entry; upgrade all nodes of a group together ([raft](release-notes-0.5.0/raft.md))
- storage-durable: `DataRoot` and `DataRootPolicy` return `DataRootError` ([storage](release-notes-0.5.0/storage.md))
- storage-durable: `LogFrame` fields are private ([storage](release-notes-0.5.0/storage.md))
- storage-durable: four public methods are deleted ([storage](release-notes-0.5.0/storage.md))
- storage-object: `ObjectMeta` and `Object` fields are private ([storage](release-notes-0.5.0/storage.md))
- storage-segment: the `SegmentStore` trait is deleted ([storage](release-notes-0.5.0/storage.md))
- storage-segment: `CatalogEntry` fields are private and checked ([storage](release-notes-0.5.0/storage.md))
- storage-segment: `ArchivedObject.version` is `ArchivedObjectVersion` ([storage](release-notes-0.5.0/storage.md))
- storage-segment: `SegmentError::ObjectStore` holds a boxed error ([storage](release-notes-0.5.0/storage.md))
- index-text: `TextDocument` fields are private and typed ([storage](release-notes-0.5.0/storage.md))
- index-text: `TextHit` ids and versions are typed ([storage](release-notes-0.5.0/storage.md))
- index-text: `TextIndex::delete` takes typed ids ([storage](release-notes-0.5.0/storage.md))
- index-text: `TextIndexSnapshot.tombstones` is typed ([storage](release-notes-0.5.0/storage.md))
- service-k8s: stateful render helpers return `Result` ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-k8s: `RenderCtx` fields are private ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-k8s: operator contract types have private fields ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-k8s: workload and RBAC plan fields are private ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-k8s: helpers removed ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-k8s: `IssuanceRequest::build` and `read_state` take a port argument ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `BackupDestination::from_uri` returns `DestinationError` ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `ScheduledBackupPolicy::to_runtime_policy` returns `PolicyError` ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `BackupSink` methods return `BackupSinkError` ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `fetch_admin_snapshot` is removed ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `BackupDestination::default_prefix` is removed ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-backup: `AdminSnapshotTransportConfig` fields are private ([service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md))
- service-auth: removed the GCE metadata ID-token source ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `async_verifier`, `role_map` and `scoped` are no longer public modules ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `reload::spawn_registry_files_watcher` removed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `Registry::parse` and `Registry::try_merge` return `RegistryError` ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `TokenClaims`, `TokenReviewOutcome` and `CachePolicy` fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `ReloadableRoleMapVerifier::reload_file` removed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-auth: `TokenRequestTarget::qualified_name` removed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-http: `HttpConfig` fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- service-http: ingest limit fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- service-http: public module paths removed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-observability: `ObservabilityConfig` fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- service-collector: `SourceProgress` fields are private and offsets are typed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-collector: `DeliveryReceipt` fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- service-collector: `RuntimeConfig` is built with `try_new`; `validate` removed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-projection: identities, cursors and generations are typed ([service-libs](release-notes-0.5.0/service-libs.md))
- service-projection: `ProjectionDescriptor` is built with `try_new` ([service-libs](release-notes-0.5.0/service-libs.md))
- service-projection: port traits return `ProjectionError` ([service-libs](release-notes-0.5.0/service-libs.md))
- service-projection: `ProjectionCheckpoint::empty` takes the time ([service-libs](release-notes-0.5.0/service-libs.md))
- service-executor: `GroupCommitConfig` fields are private ([service-libs](release-notes-0.5.0/service-libs.md))
- claim-token: `Scope` fields are private and keys are typed ([service-libs](release-notes-0.5.0/service-libs.md))
- server-http: `HttpServerOptions` and `TlsServerOptions` fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- server-http: `HttpServerReport` is a type alias ([server-transport](release-notes-0.5.0/server-transport.md))
- server-tcp: `TcpHandler` futures resolve to `TcpHandlerError` ([server-transport](release-notes-0.5.0/server-transport.md))
- server-tcp: config, socket and connection-result fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- server-lifecycle: `BindConfig` and `ShutdownDeadline` fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- transport-h2c: `ManagerConfig` and `ConnectionOptions` fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- transport-otlp: `DecodedPayload::signal()` removed ([server-transport](release-notes-0.5.0/server-transport.md))
- peer-tls: `PeerTlsConfig` fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- metrics-prometheus: sample, bucket and latency fields are private ([server-transport](release-notes-0.5.0/server-transport.md))
- metrics-remote-write: `ValidatedWrite::request()` removed ([server-transport](release-notes-0.5.0/server-transport.md))
- openapi-codegen: root modules and the `target` compat facade ([tooling](release-notes-0.5.0/tooling.md))
- openapi-codegen: `GenOptions` fields are private and built with builders ([tooling](release-notes-0.5.0/tooling.md))
- openapi-codegen: `TargetProfile::default_for` removed ([tooling](release-notes-0.5.0/tooling.md))
- openapi-codegen: emitters and parsers return typed errors ([tooling](release-notes-0.5.0/tooling.md))
- openapi-codegen: generated Rust clients return typed errors for declared failures ([tooling](release-notes-0.5.0/tooling.md))
- openapi-codegen: generated Rust oneOf/anyOf schemas are serde enums, not serde_json::Value ([tooling](release-notes-0.5.0/tooling.md))
- compass: root modules `checker`, `format`, `outline`, `refactoring`, `search` and `watch` removed ([tooling](release-notes-0.5.0/tooling.md))
- compass: unused items deleted ([tooling](release-notes-0.5.0/tooling.md))
- compass: `LintConfig` fields are private ([tooling](release-notes-0.5.0/tooling.md))
- compass: `Diagnostic::code` is `RuleCode`, not `String` ([tooling](release-notes-0.5.0/tooling.md))
- compass: `SymbolId` and `ScopeId` fields are private; `RuleCode` replaces `String` in two methods ([tooling](release-notes-0.5.0/tooling.md))
- compass: `Range::from_node` replaced by `NodeRange` ([tooling](release-notes-0.5.0/tooling.md))
- compass: rejected custom rules are returned as data ([tooling](release-notes-0.5.0/tooling.md))
- cli-std: `llm::Topic`, `SectionedTopic` and `ToolInfo` fields are private ([tooling](release-notes-0.5.0/tooling.md))
- cli-std: option structs are built with builders ([tooling](release-notes-0.5.0/tooling.md))
- cli-std: three functions removed ([tooling](release-notes-0.5.0/tooling.md))
- surface: `Props` and `Component` fields are private ([tooling](release-notes-0.5.0/tooling.md))
- ui-runtime: `FiberId` field is private and debug API takes `FiberId` ([tooling](release-notes-0.5.0/tooling.md))

## Fixes apps must take

- raft-runtime: `propose` no longer reports `Completed` for a superseded entry (#102) ([raft](release-notes-0.5.0/raft.md)).
- raft-runtime: a truncated forwarded proposal is `Ambiguous`, not `Completed` (#103) ([raft](release-notes-0.5.0/raft.md)).
- compass: slice test-extractor arguments on byte offsets (`d514f26a`) ([tooling](release-notes-0.5.0/tooling.md)).
- openapi-codegen: optional untyped Rust field gets `Option::is_none` on `serde_json::Value` (#130, `b41c6a3f`) ([tooling](release-notes-0.5.0/tooling.md)).

## New

- [raft](release-notes-0.5.0/raft.md): the `RaftStorage` and `RaftDelivery` ports, read rounds and peer match indexes.
- [storage](release-notes-0.5.0/storage.md): typed constructors and accessors for the durable, segment, object and text index crates.
- [service-k8s-backup](release-notes-0.5.0/service-k8s-backup.md): backup CronJob and rollout planners, and `BackupSetV1` and `RestoreLedger`.
- [service-libs](release-notes-0.5.0/service-libs.md): projected-token mode, typed offsets and projection names.
- [server-transport](release-notes-0.5.0/server-transport.md): `TcpHandlerError`, builders and getters for server, transport and metrics config.
- [tooling](release-notes-0.5.0/tooling.md): codegen API paths, typed compass rules and cli-std builders.

## Crates checked

### raft

- `raft-core`
- `raft-runtime`

### storage

- `storage-durable`
- `storage-segment`
- `storage-object`
- `index-text`

### service-k8s-backup

- `service-k8s`
- `service-backup`

### service-libs

- `service-auth`
- `service-http`
- `service-collector`
- `service-observability`
- `service-projection`
- `service-executor`
- `service-mcp`
- `claim-token`

### server-transport

- `server-http`
- `server-lifecycle`
- `server-tcp`
- `transport-h2c`
- `transport-otlp`
- `peer-tls`
- `metrics-prometheus`
- `metrics-remote-write`

### tooling

- `openapi-codegen`
- `compass`
- `cli-std`
- `surface`
- `ui-runtime`
- `build-stamp`
