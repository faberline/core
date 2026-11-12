# ADR 0002: Core-owned service operations

- **Status:** Accepted on 2026-11-11. Phase 0 (this ADR and the boundary
  rewrite in the service-k8s documents) changes documentation only. No code
  has changed.
- **Date:** 2026-11-05
- **Applies to:** `crates/service-k8s`, `crates/raft-runtime`, a new
  `crates/shard-routing`, and the downstream operators built on them. The S5
  service apps (defer, keep, lumen, relay, tape) move first; loom, sift and
  pgpool follow once the engine has a profile for their shape.

## Context

Eight downstream apps build their operators on service-k8s. Five of them
(defer, keep, lumen, relay, tape) are the same class of product, but each has
its own `-operator` crate and writes its own operations. Those five crates hold
about 11.7k lines: lumen about 8.2k, tape 1.8k, and defer, keep and relay about
0.6k each.

- **Most operator code is copied.** All eight apps write their own phase
  logic, probes and Prometheus annotations, operator install YAML and RBAC.
  defer, tape and lumen carry the same placement types, down to identical doc
  comments. The backup command line, peer-TLS settings and CRD normalization
  each exist in three copies.
- **`ClusterSpec` covers little.** It holds image, pull policy, shard, replica
  and voter counts, and CPU and memory. Every app defines its own storage,
  auth, TLS, backup, placement and pull-secret fields.
- **There are two renders.** keep, relay, defer, tape, loom and lumen also
  hand-write a static manifest (`k8s/base`, `k8s/components`) that duplicates
  the operator's output. Every overlay except lumen's GKE test overlay applies
  that static YAML without an operator.
- **Apps schedule the same outcome separately.** lumen's
  `membership-aware-replica-autoscaling` is tape's `live-replica-membership`;
  lumen's `high-availability-shard-expansion` is tape's
  `multi-shard-topology`.
- **Core mechanisms have no driver.** No app calls `plan_replica_layer` or
  `plan_shard_split` (`crates/service-k8s/src/domain/capacity.rs`). Nothing
  drives the raft-runtime membership operations
  (`crates/raft-runtime/src/application/host/membership_ops.rs`). Nothing uses
  `RaftRegistry` (`crates/raft-runtime/src/application/registry.rs`); keep
  wrote its own `ShardHosts` instead. lumen's operator reimplements core's
  private pod-identity code and opens four more leader Leases for its
  background loops.
- **The documents told apps to own policy.** The service-k8s roadmap listed
  `service-specific-topology-policy` as a non-goal. `CONTRIBUTING.md` said
  routing policy remains in the service. lumen's `CONTRIBUTING.md` keeps shard
  and health policy in lumen and forbids moving its protected reshard fields
  into a generic Fleet policy. Each app therefore had to build what the others
  also needed.
- **The topology itself blocks scaling.** Every shard runs in one StatefulSet
  and a pod's shard is `ordinal % SHARD_COUNT`. As a result a shard cannot
  split while replicas per shard exceed one, an HPA cannot target the workload
  (lumen retired `spec.serving.autoscaling` for this reason; see its
  `docs/migration-2680-retire-serving-autoscaling.md`), and a new shard map
  needs a rolling restart because pods read it once from the environment at
  boot.

## Decision

Core decides how a service runs. An app supplies two things: a declaration of
what it is, and the data-plane hooks that only its domain can implement. The
numbered decisions below are cited by the downstream roadmaps and by the
service-k8s [roadmap](../../crates/service-k8s/ROADMAP.md).

### Boundary

- **D1 — Core owns operational policy.**

  | The app owns | Core owns |
  |---|---|
  | Product API, the domain state machine (`RaftStateMachine` impl), storage format | Topology: shard count, voters and learners per shard, the shape of StatefulSets, Services, PDBs and NetworkPolicies |
  | Product config schema (`type Config`, placed in the CR's `spec.config`) | Scaling: when to add or remove learners, when to split, how HPA and KEDA attach |
  | Request classification (keyed, keyless, namespace lifecycle) and the merge of scattered results | Routing: the bucket map, forwarding, map versions, the write fence, scatter-gather execution |
  | Per-bucket data usage, export, import and pruning (hooks) | The reshard flow: fence, two copy passes, prune, checkpoint, cutover, convergence, rollback |
  | Domain readiness dimensions (for example searchable) and domain alert rules, supplied as data | Conditions and gating, rollout order, quorum safety |
  | Access: which verb each operation needs, the token audience, anonymous routes | Certificates, client trust, auth delegation, backup schedules, retention, Fleet, placement, monitoring |

- **D2 — The decision rule.** Code that talks about Kubernetes, Raft
  membership, a shard map or forwarding between pods belongs in core. Code
  that talks about documents, search, topics, messages, jobs or keys belongs in
  the app. An app cannot override core's render or reconcile. It chooses
  capabilities from a closed set that core defines, and a missing capability
  is added to core, not built in one app.
- **D3 — One operator process per app.** Each app keeps its own operator
  process and its CRD group and kind; the process runs core's engine. A
  profile can be serialized (`<app> k8s profile` prints it as JSON), so a
  later cluster-wide shared operator needs no app change.

### Profile contract

- **D4 — `ServiceProfile` is the only operational code an app writes.** It
  lives in the app's assembly crate as `crates/<repo>/src/profile.rs`:

  ```rust,ignore
  impl ServiceProfile for Lumen {
      const KIND: &'static str = "Lumen";   // CRD group and kind stay the same
      const PREFIX: &'static str = "LUMEN"; // environment prefix
      type Config = LumenConfig;            // product config; core never interprets it
      fn capabilities() -> Capabilities {
          Capabilities {
              partitioning: Partitioning::Bucketed, // others start with Single
              read_scaling: ReadScaling::Learners,  // leader-bound apps use None
          }
      }
      fn alerts() -> Vec<AlertRule> { /* domain alert rules, as data */ }
  }
  ```

  The capability set starts closed at two axes:

  | App | `partitioning` | `read_scaling` |
  |---|---|---|
  | lumen | `Bucketed` | `Learners` |
  | tape, defer | `Single`, then `Bucketed` when their multi-shard outcomes ship | `None` |
  | relay, keep | `Single` | `None` |

- **D5 — Product config reaches the pod through core.** Core flattens
  `spec.config` into `<PREFIX>_*` keys in a ConfigMap and injects it with
  `envFrom`. Secret fields use core's `SecretRef` type, and core mounts them as
  files. This removes the class of defect where an operator writes a
  ConfigMap key that never reaches the pod.
- **D6 — Core owns the operational spec blocks** that each app defines today:
  image and resources; storage (size, class, resize); replicas, voters, scale
  and sharding policy; auth (mode, token registry, SubjectAccessReview
  delegation); peer TLS and certificates; backup (policy, schedule, admin
  token); placement (node selector, tolerations, affinity, topology spread,
  capacity catalog); monitoring (ServiceMonitor, the standard alerts plus the
  profile's `alerts()`); and image pull secrets.
- **D7 — Standard conditions.** Every instance reports tape's current set
  (`Ready`, `Progressing`, `StorageHealthy`, `BackupConfigured`,
  `BackupSucceeded`) plus the dimensions its `DomainReadiness` hook reports.
- **D8 — Data-plane hooks.** The app implements them in its server process,
  and core mounts them on standard admin routes. The list comes from the pod
  endpoints that lumen's reshard driver calls today.
  - `RequestRouting`: `classify(&req)` returns `Keyed { namespace, key }`,
    `Keyless` (scatter), `NamespaceLifecycle` or `NamespaceWideWrite` (both
    broadcast); `merge(responses)` combines scattered results.
  - `BucketData`, required only for `Bucketed`: `usage_bytes(bucket)`;
    `export(buckets)` with each key and its encoded size; `import(batch)`,
    idempotent and additive only; `retain_only(bucket, keys)`;
    `evict_unowned(map)`; `checkpoint()`, which returns once data is
    persisted; and `list_namespaces()`.
  - `DomainReadiness`: the domain dimensions for D7.

### Topology and scaling

- **D9 — One StatefulSet per shard.** Shard 0 keeps the `<name>` StatefulSet,
  so a single-shard instance keeps its PVC names (`raft-<name>-<r>`). Shard
  `N` is `<name>-s<N>`. Each shard gets a headless Service, and one client
  Service spans every shard. The renderer injects `SHARD_ID`; a pod's replica
  is its ordinal. raft-runtime stops deriving the shard from
  `ordinal % SHARD_COUNT` in the downward-API quartet
  (`crates/raft-runtime/src/app/topology_env.rs`).
- **D10 — Membership comes from the Raft log.** Only the initial members
  bootstrap from the boot environment. A pod added later boots as a joiner,
  waits until the controller adds it as a learner, and catches up from a
  snapshot. StatefulSet PVCs use the `Retain` retention policy, and the
  controller deletes an ordinal's PVC only after that member's removal has
  committed. A manual scale-in therefore cannot destroy a voter's vote record.
- **D11 — Replica scaling goes through membership first.** The CR gets a
  `scale` subresource. `spec.replicas` is replicas per shard, and every shard
  moves one whole layer at a time (`plan_replica_layer`). `status.selector`
  covers every pod. HPA and KEDA target the CR, never a StatefulSet, and core
  completes the membership change before it changes the pod count. This is
  what lumen's retired `spec.serving.autoscaling` lacked.
  - `ReadScaling::Learners`: voters stay at `spec.voters`, every extra replica
    is a learner, and HPA adds or removes learners only.
  - `ReadScaling::None`: every replica is a voter and there is no scale
    subresource. Scaling out runs add learner, catch up, promote. Scaling in
    transfers leadership first, then demotes and removes the member.
- **D12 — Shard count belongs to core's reshard controller.** It splits when
  `usage_bytes` crosses the `plan_shard_split` thresholds; HPA never changes
  shard count. This is the two-axis rule in the "HA — `raft-core`, sharded
  and strongly consistent" section of [CONTRIBUTING](../../CONTRIBUTING.md).
  Only `Bucketed` services split. A split adds a StatefulSet and does not
  depend on replicas per shard, so the two capabilities are independent.
- **D13 — The shard map is a controller-owned child.** It lives in the
  ConfigMap `<name>-shard-map`, not in the user's spec. (lumen writes
  `spec.shardMap` today, which fights any GitOps tool that applies the spec.)
  Pods mount the ConfigMap, reload it without a restart, and report the map
  version they serve on `/metrics`. The convergence gate keeps working without
  a rolling restart. A lost ConfigMap is rebuilt from the map the pods report.
- **D14 — Static mode.** `<app> k8s render --static` renders one shard with a
  fixed replica count, using the same renderer as the operator, and the
  output is committed as `k8s/base`. Every change to replicas or shards goes
  through the control plane: a CR applied directly, or a Fleet entry. A
  manual `kubectl scale` in static mode starts extra pods as joiners that stay
  not ready, so it cannot split the group.

### Routing and resharding

- **D15 — Routing is a shared data-plane crate.** `crates/shard-routing` moves
  out of lumen's `crates/lumen-index/src/sharding/`: the virtual-bucket shard
  map (`bucket_of`, `balanced`, `split_one_shard`, `moves`), the routed router
  (one-hop forwarding and scatter), and the write fence (server state, guard
  and client protocol). Its headers get neutral names
  (`x-faberline-forwarded`, `x-faberline-map-version`), and pods accept both
  the old and the new names during migration. keep's `crc32(key) % shard_count`
  is the special case where the virtual bucket count equals the shard count.
- **D16 — The reshard flow is core's.** The reshard controller generalizes
  lumen's `crates/lumen-operator/src/application/reshard_driver/`; its
  `ClusterControl` (`cluster_control.rs`) becomes generic over the CR type.
  Core runs the fence, two copy passes, prune, checkpoint, cutover,
  convergence and rollback; the app's `BucketData` hook moves the bytes.

### What core adds

- **D17 — service-k8s.**
  - The `ServiceProfile` engine on top of `ManagedService`
    (`src/application/operator/managed_service.rs`): a generic CRD, render and
    reconcile, and the operator install manifests (Deployment, CRD, and RBAC
    derived from the children it actually renders).
  - The pieces each app copies today: hardening and probes, placement types,
    the backup wrapper and command line, peer TLS, CRD normalization and phase
    logic.
  - A per-shard StatefulSet render that replaces `sharded_statefulset`
    (`src/infrastructure/manifest/workloads.rs`).
  - One leader Lease shared by every background loop, and a public
    pod-identity API.
  - The membership controller, driving raft-runtime's membership operations.
  - The reshard controller (D16).
  - Scaling: `stateful::plan_*` wired into the controllers, and lumen's HPA
    handoff (`crates/lumen-operator/src/application/reconcile/hpa.rs`) moved
    into core.
  - Fleet, extracted from lumen's `application/fleet_reconcile.rs` as the
    `shared-fleet-controller` outcome already planned.
  - Corrected documents that still say membership is static: the
    `src/domain/capacity.rs` module doc and the `ClusterSpec` field docs in
    `src/interfaces/cluster_spec.rs`.
- **D18 — raft-runtime.** `SHARD_ID` topology, joiner boot, and membership
  admin routes for the controller.

### What the app keeps

- **D19 — The app remainder.** The `<repo>-operator` crate goes away. The
  `<repo>` assembly crate gains `profile.rs` and the hook implementations.
  `k8s/operator/*.yaml` and `k8s/base` are generated by `<app> k8s …`,
  committed, and guarded by drift tests, the same `include_str!` gate the CRD
  already has. The aw S5 standard records the target shape; its checker
  changes when the first app deletes its `-operator` crate (Phase 3).

### Phases

- **D20 — Milestones.**

  | Phase | Core delivers | App side |
  |---|---|---|
  | Prerequisite | A core tag. Core HEAD already made the fields of `ReadinessTarget`, `ReadyFacts`, `ReconcilePlan` and `PruneTarget` private. | All eight apps build these types with struct literals and switch to constructors when they bump. The change is mechanical and does not wait for the engine. |
  | 1 | The `ServiceProfile` engine for one shard: generic CRD, render, reconcile, install manifests and `--static`, covering the spec blocks of D6 and the conditions of D7. | lumen proves with its `operator_render` tests that its output does not change, then deletes its own render layer; `k8s/base` becomes generated. |
  | 2 | Per-shard topology, `SHARD_ID`, joiner boot. | Single-shard lumen and tape instances move without copying data. |
  | 3 | The membership controller, the scale subresource, PVC cleanup. | tape moves onto the engine, deletes `tape-operator` and delivers `live-replica-membership`; lumen delivers `membership-aware-replica-autoscaling`; aw changes the S5 checker. |
  | 4 | `shard-routing`, the bucket hooks, the shard-map child with hot reload, the reshard controller. | lumen splits at any replica count. lumen adds per-bucket byte accounting; it balances by bucket count today. |
  | 5 | lumen's remaining background controllers: Fleet, rollout, certificate rotation, client trust, retention, capacity catalog. | lumen deletes `lumen-operator`. |
  | 6 | — | defer, then relay, then keep move onto the engine and delete their `-operator` crates; keep replaces `ShardHosts` with `RaftRegistry`. This needs only Phases 1–3. |
  | Later | Multi-workload and Deployment profiles. Sift needs a StatefulSet profile with two fixed 3-voter Raft groups (store and control), a collector workload, a GCS archive, and `PrometheusRule` and `ServiceMonitor` children; see `multi-workload-service-profile` and `prometheus-monitoring-children` in the service-k8s roadmap, `archive-lifecycle-primitives` in the storage-segment roadmap, `scratch-object-store` in the storage-object roadmap and `backup-cronjob-render-helper`. | loom, sift and pgpool. tape and defer switch to `Bucketed` when they deliver their multi-shard outcomes. |

### Migration

- **D21 — No in-place conversion for multi-shard instances.** No long-running
  lumen instance with `shardCount > 1` is known; only the GKE test matrix
  creates them, temporarily. Core does not convert them in place. An old
  multi-shard instance is rebuilt through backup and restore.
- **D22 — Migration risks and their handling.**
  - A StatefulSet selector is immutable. Adding the shard label to shard 0
    means orphan-deleting and recreating the StatefulSet object; the
    controller does this, and the PVCs stay.
  - lumen's `shardCount`, `replicasPerShard`, `voterCount`, `reshardPolicy`
    and `shardMap` fields need aliases or a conversion. `spec.shardMap` moves
    to the shard-map child on the first reconcile. Doc comments feed
    `crd.yaml` and `openapi.json`, so moved types keep their doc comments.
  - lumen's static base is a Deployment with `emptyDir`. The generated
    StatefulSet uses a PVC, so every lumen overlay (dev, staging, prod) needs
    a usable StorageClass. No data moves.
  - lumen's static overlays use `SHARD_COUNT` for the number of in-pod
    partitions, the same name as the topology variable. When the quartet
    retires, lumen moves to its own key and keeps reading the old name for a
    while.
  - lumen's release-candidate workflow runs three core tests by name (see
    [operations](../operations/README.md)). Renaming them disables that check
    silently, so a core bump updates those commands and checks for a nonzero
    test count.
  - Old axiom finalizer names (`service-k8s.axiom.dev/...`) and the new names
    coexist while they are renamed.
  - GitOps and HPA overwrite each other. A GitOps manifest for an instance
    under HPA or KEDA must not set `spec.replicas`; the deployment documents
    say so.

## Consequences

- An app writes a profile and its hooks. Its operator crate, between 0.6k and
  8.2k lines today, goes away.
- Core takes on behavior it used to leave to apps: topology transitions,
  membership, resharding and routing. A defect there affects every app at
  once, so each outcome carries end-to-end evidence from a real consumer: tape
  scales 1→3→1 while publishing with no committed message lost and no
  surviving member restarted; lumen scales learners 3→5→3 under HPA without
  interrupting search; and lumen splits once at three replicas per shard with
  every document still found.
- The service-k8s roadmap drops the `service-specific-topology-policy`
  non-goal, rewrites its outcome boundaries to match D1, and adds the outcomes
  that implement D4–D18. Downstream roadmaps adopt those IDs instead of
  scheduling their own copies.
- The aw S5 checker changes in Phase 3, not now. Before the engine exists, no
  repo could pass the new checks.
- pgpool and sift keep using `ManagedService` directly until the engine has a
  profile for a Deployment or for several workloads. Sift's needs are tracked as
  the Later outcomes named in D20.
- Defects found while surveying the operators are tracked as separate issues.
  They do not depend on this ADR.
