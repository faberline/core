# service-k8s roadmap

## Purpose

This document records future shared-library outcomes and explicit non-goals. It
does not describe current support. [STATUS.md](STATUS.md) owns that contract.

The issue tracker owns assignees, work state, schedules, and delivery history.
Stable IDs here let current limits point to one destination.

[ADR 0002](../../docs/adr/0002-core-owned-service-operations.md) sets the
boundary every outcome below follows: core owns how a service runs, and an app
supplies domain facts through its profile and data-plane hooks. Downstream
roadmaps adopt these IDs instead of scheduling their own copies. The ADR's
phase table orders the outcomes.

## Near-term outcomes

### Service profile engine

- ID: `service-profile-engine`
- Outcome: An app declares a `ServiceProfile` (kind, environment prefix,
  product config type, capabilities, and domain alert rules) and gets a generic
  CRD, render, reconcile, standard conditions, operator install manifests, and
  a static single-shard render from it.
- Boundary: Core owns the spec blocks (image, resources, storage, replicas,
  voters, auth, peer TLS, backup, placement, monitoring, and pull secrets), the
  render, the reconcile order, and the condition set. The app supplies its
  product config, which core injects without interpreting it, its domain
  readiness dimensions, and its alert rules as data. Capabilities come from a
  closed set, and the app cannot override render or reconcile. The first
  release supports one shard.
- Completion evidence: Library tests prove that every `<PREFIX>_*` config key
  reaches the pod, secret references are mounted as files, operator RBAC is
  derived from the rendered children, conditions include the domain
  dimensions, the static render uses the same renderer as the operator, and a
  capability outside the closed set is refused. Lumen's `operator_render`
  tests prove its rendered children do not change when it moves onto the
  engine.
- Tracking: Not assigned.

### Per-shard topology

- ID: `per-shard-topology`
- Outcome: Each shard runs as its own StatefulSet with its own headless
  Service, and a pod learns its shard from `SHARD_ID` instead of deriving it
  from its ordinal.
- Boundary: Core owns workload names, selectors, labels, the client Service
  that spans every shard, and the migration of existing instances. Shard 0
  keeps the name `<name>` and its PVCs `raft-<name>-<r>`; shard N is
  `<name>-s<N>`. raft-runtime reads `SHARD_ID` and boots a pod that is not in
  the initial membership as a joiner. Apps supply nothing.
- Completion evidence: Render tests cover one shard and several shards.
  Migration tests prove that recreating the shard-0 StatefulSet with the shard
  label keeps its PVCs and data. A runtime test proves a joiner stays not
  ready until the controller adds it.
- Tracking: Not assigned.

### Membership controller

- ID: `membership-controller`
- Outcome: Changing the replicas per shard changes Raft membership before it
  changes pods, and HPA or KEDA can drive the change through the CR's scale
  subresource.
- Boundary: Core owns add learner, catch up, and promote; transfer leadership,
  demote, and remove; whole-layer planning across shards; the voter count;
  PVC deletion after a member's removal commits; and the scale subresource.
  With `ReadScaling::Learners`, `spec.voters` stays fixed and scaling adds or
  removes learners. With `ReadScaling::None`, every replica is a voter and
  there is no scale subresource. The app supplies the domain readiness that
  marks a new member as caught up.
- Completion evidence: Library and runtime tests prove that no pod changes
  before the membership change commits, leadership moves before the leader is
  removed, PVCs stay until removal commits, an interrupted transition recovers,
  and a controller leader failover resumes the transition. Tape scales
  1→3→1 under publish load without losing a committed message or restarting a
  surviving member. Lumen's HPA scales learners 3→5→3 without interrupting
  search.
- Tracking: Not assigned.

### Bucketed resharding

- ID: `bucketed-resharding`
- Outcome: A `Bucketed` service adds a shard automatically when durable bytes
  cross a threshold, at any replica count.
- Boundary: Core decides when to split from `usage_bytes` and the
  `plan_shard_split` thresholds. It owns the shard-map ConfigMap and its hot
  reload, and it runs the fence, both copy passes, prune, checkpoint, cutover,
  convergence, and rollback. The app implements the `BucketData` hooks. HPA
  never changes the shard count.
- Completion evidence: Controller tests prove the split decision, fence and
  cutover order, rollback from each step, convergence on the map version,
  leader failover mid-flow, and rebuilding a lost shard-map ConfigMap from pod
  reports. Lumen splits once at three replicas per shard and every document
  stays findable.
- Tracking: Not assigned.

### Shard routing

- ID: `shard-routing`
- Outcome: Services route, scatter, and fence requests across shards through
  one shared library.
- Boundary: A new `crates/shard-routing` owns the virtual-bucket map, one-hop
  forwarding, scatter-gather, map-version headers, and the write fence. The
  app's `RequestRouting` hook classifies each request and merges scattered
  results. Header names are neutral (`x-faberline-forwarded`,
  `x-faberline-map-version`); during migration the library also accepts
  Lumen's current names. The crate starts from the sharding module in Lumen's
  `lumen-index`.
- Completion evidence: Library tests prove bucket assignment, balanced and
  split maps, bucket moves, no forwarding loops, scatter with an app merge,
  the write fence, refusal of a stale map version, and acceptance of both
  header names. Lumen's sharding tests pass against the shared crate.
- Tracking: Not assigned.

### Shared Fleet controller

- ID: `shared-fleet-controller`
- Outcome: The engine runs a watch-driven multi-namespace Fleet controller
  with reusable ownership, adoption, prune, rollout, and status-projection
  mechanics.
- Boundary: Core owns Fleet ownership, adoption, prune, rollout order, health
  gating, and status. A Fleet entry is a profile instance, so child identity,
  seed objects, and protected fields (such as the shard map) come from the
  engine, not from the app. The app supplies only domain readiness dimensions.
  The first consumer preserves Lumen's existing materialization behavior
  before it adds readiness and rollout behavior.
- Completion evidence: Library tests prove duplicate-target rejection,
  ownership and adoption safety, Retain and Delete prune behavior, Fleet and
  child watches, low-frequency repair, per-entry API failure isolation, status
  projection, ordered concurrency, and leader failover. Lumen adapter tests
  prove parity for current child objects and protected reshard fields.
- Tracking: Not assigned.

### Declarative access RBAC convergence

- ID: `declarative-access-rbac-convergence`
- Outcome: Apps can reconcile a typed set of namespaced ServiceAccount access
  grants through reusable Role, RoleBinding, ownership, prune, and status
  mechanisms.
- Boundary: Core owns object identity, explicit ServiceAccount subjects,
  apply, adoption, prune, status facts, and replacement rules. The app
  supplies domain facts: its access CRD, the token audience, and the verb each
  operation needs. Core does not create namespaces, ServiceAccounts, client
  workloads, or token Secrets.
- Completion evidence: Library tests prove cross-namespace ServiceAccount
  subjects, exact named rules, explicit deny-all as an empty desired grant set,
  create and update, ownership refusal, adoption, stale RoleBinding prune,
  per-object failure isolation, status projection, wildcard refusal, and no
  Secret rendering. App tests prove their operation-to-verb mapping without
  adding it to the shared library.
- Tracking: Not assigned.

### Public trust-bundle convergence

- ID: `public-trust-bundle-convergence`
- Outcome: Apps can publish public CA bundles into declared namespaces through
  reusable ConfigMap ownership, adoption, rotation-overlap, prune, and status
  mechanics.
- Boundary: Core owns the public bytes, the ConfigMap name and labels, runtime
  identity, old-and-new root overlap, retention, readiness, and generic object
  convergence. The instance spec declares the target namespaces. Core does not
  create namespaces, ServiceAccounts, client Deployments, or token Secrets,
  and the deployment supplies CA policy.
- Completion evidence: Library tests prove multi-namespace create and update,
  shared desired objects, exact owner identity, adoption refusal, old-and-new
  root overlap, stale owned-object prune, foreign-object retention, per-object
  failure isolation, status facts, public-data-only validation, and leader
  failover.
- Tracking: Not assigned.

### Failure-domain placement primitives

- ID: `failure-domain-placement-primitives`
- Outcome: An instance spec can declare typed selectors, tolerations, topology
  intent, and group-scoped hard anti-affinity without raw Kubernetes
  expressions.
- Boundary: Core derives the group identity (one shard) from its own topology
  and decides host or zone separation from the instance's placement spec. It
  validates label selectors and topology keys, then renders standard
  Kubernetes scheduling fields. The placement block also carries the capacity
  catalog that ADR 0002 D6 gives core: each named class maps to typed
  selectors and tolerations, and core treats the class name as opaque data.
  A catalog may name its classes after machine types, as lumen's does today,
  but the shared contract stays free of cloud-specific fields such as GKE
  machine types, ComputeClasses, and node-pool lifecycle. Apps supply
  nothing.
- Completion evidence: Pure render tests prove group-scoped host anti-affinity,
  required and preferred zone spread, selectors, tolerations, a catalog class
  resolving to its selectors and tolerations, deterministic labels, allowed
  node sharing by different groups, malformed-input refusal, and no
  cloud-specific field in the shared contract.
- Tracking: Not assigned.

### Controlled StatefulSet member rollout

- ID: `controlled-statefulset-member-rollout`
- Outcome: The engine updates one StatefulSet member at a time and moves to the
  next only when the member is healthy.
- Boundary: Core owns partition and member selection, generation tracking,
  pause and resume, recovery, status facts, and the gates that permit the next
  member: group membership, quorum, replication readiness, and ordering
  (followers before the leader, with leadership transferred first). The app
  supplies domain readiness dimensions such as searchable. A PDB remains an
  eviction control; the rollout mechanism does not treat it as a StatefulSet
  update gate.
- Completion evidence: Library fixtures prove one-at-a-time selection,
  generation gating, quorum, replication, and domain-readiness gating,
  leader-last ordering with leadership transfer, pause and resume, leader
  failover, interrupted reconcile recovery, no second member while the first
  is blocked, and explicit separation between voluntary-eviction policy and
  workload rollout.
- Tracking: Not assigned.

### Managed certificate lifecycle

- ID: `managed-certificate-lifecycle`
- Outcome: The engine requests, owns, and rotates separate serving and peer
  leaf certificates for each instance.
- Boundary: Core composes the existing certificate module. It derives serving
  identities from the instance's Service DNS names and peer identities from
  its own topology, stores leaves in runtime Secrets, keeps serving and peer
  material separate, and gates readiness on valid material. The deployment
  supplies the issuer, CA policy, and workload identity permission. Private
  keys never enter Fleet, status, Events, or logs. Pre-created Secrets remain
  a compatibility path. Apps supply nothing.
- Completion evidence: Tests prove identity derivation, the first issue for
  each member, Secret separation, renewal before expiry without quorum loss,
  issuer outage and recovery, hot reload, and refusal of an identity outside
  the instance scope.
- Tracking: Not assigned.

### Instance data retention

- ID: `instance-data-retention`
- Outcome: Deleting a Fleet child and deleting an instance's data become
  separate, explicit policies that default to Retain, and member removal
  follows the same rule.
- Boundary: Core owns the retention field, the finalizer, selecting only the
  instance's own PVCs, deleting a member's PVC only after its removal commits,
  and backup retention through `service-backup`. The StorageClass reclaim
  policy still decides the PersistentVolume lifecycle. Apps supply nothing.
- Completion evidence: Tests prove that default deletion leaves PVCs, Delete
  removes only the instance's exact PVCs, unrelated PVCs remain, failed cleanup
  keeps the finalizer and publishes a condition, a retry succeeds, a removed
  member's PVC stays until its removal commits, and backups expire by
  `retentionSecs`.
- Tracking: Not assigned.

## Later outcomes

No items.

## Non-goals

### Namespace and cluster provisioning

- ID: `namespace-and-cluster-provisioning`
- Reason: Cluster and namespace lifecycle includes platform policy, quota,
  identity, and ownership. Fleet mechanics operate inside supplied scopes.

### App render override

- ID: `app-render-override`
- Reason: Apps choose capabilities from a closed set. An app that needs a
  different shape extends core, so every app keeps the same operational
  behavior.

### Workload-level autoscaling

- ID: `workload-level-autoscaling`
- Reason: An HPA or KEDA scaler that targets a StatefulSet directly changes
  pods without a membership change. Autoscalers target the CR's scale
  subresource instead, so membership changes first.
