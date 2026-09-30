# service-k8s

service-k8s is the Kubernetes operator kit. A service supplies its custom
resource type, rendered children, readiness targets and status meaning; the kit
watches the resource, holds a leader lease, applies children with server-side
apply, prunes unwanted ones and projects status conditions. It also renders
workloads and RBAC roles, validates termination budgets, plans stateful
capacity and PVC growth, and runs a certificate lifecycle. It never defines a
service's schema, access policy, topology or health meaning. Eight downstream
apps build their operators on it: defer, keep, loom, lumen, pgpool, relay, sift
and tape.

**Form:** layered · **Depends on:** cli-std, metrics-prometheus (both only with the `controller` feature) · **Crate:** [`crates/service-k8s`](../../crates/service-k8s)

## Model

- **Managed service** — a custom resource type implementing `ManagedService`.
  Per reconcile it supplies a `ReconcilePlan` (children plus an opaque
  context), `ReadinessTarget`s, a status patch, `ConditionFact`s,
  `PruneTarget`s and `ClusterScopedChild`ren.
- **Condition** — a status condition in the metav1 shape; `project` builds
  `Condition`s from `ConditionFact`s and a time the caller passes in. The
  operator's application step reads that time (`now_rfc3339`), adds the
  controller's own `PruneBlocked` condition and writes the result into the
  status patch.
- **leader lease** — the Lease named by the service's `MANAGER`, which is also
  its field manager. `Election` records whether this replica holds it;
  `may_acquire` is the rule for taking it over. A reconcile pass acts only
  while its leadership says "leader": the operator campaigns for the Lease,
  while a one-shot pass (`reconcile_once`) takes the caller's `Election` as
  given and never promotes it.
- **Termination budget** — a `LifecyclePolicy` validated into a
  `TerminationBudget` that fits inside the pod's grace period.
- **Capacity plan** — `plan_replica_layer` scales whole replica layers (one
  replica per shard); `plan_shard_split` plans one new shard from durable bytes;
  a `ResizeAction` says whether a PVC grows.
- **Workload plan** — typed `*Plan` values that render to manifests, RBAC roles
  included.
- **Certificate profile** — what a service asks a certificate for, checked
  against an `InstanceScope`; `next_action` picks the next `Action` from the
  observed state and the current time.
- **Secret layout** — the Secret a certificate is projected into
  (`material_secret`, `trust_bundle_secret`) and the `ProjectedState` read back
  out of it (`read_state`). Pure domain; reading the stored leaf goes through
  `LeafParser`.

## Ports

- `ManagedService` — implemented by each downstream operator's CRD root type.
  It and its plan, readiness and child types sit in the application layer,
  next to the condition step that stamps what it returns.
- `Issuer` — signs a CSR; `EphemeralIssuer`, `CasIssuer`.
- `KeyAndCsrGenerator` — a fresh keypair and a CSR for a profile, used by
  `IssuanceRequest::build`; `RcgenCsrGenerator`.
- `LeafParser` — validity and fingerprint of a stored PEM leaf, used by
  `read_state`; `X509LeafParser`.
- `SecretStore` — a certificate's Secret; `KubernetesSecretStore`, `MemoryStore`.
- `LeaderLease` — acquires and renews the leader Lease for an `Election`;
  the kube Lease loop (`lease::spawn`).
- `AccessTokenSource` — the CA Service token; GKE metadata, workload identity.

The composition root, `src/app/`, keeps three public entry points with
their signatures unchanged:
- `Reconciler::new(scope, owner, store, issuer)` wires `RcgenCsrGenerator`
  and `X509LeafParser` into the certificate reconciler.
- The operator's `run` builds the kube client and the Lease adapter,
  campaigns for leadership and starts the controller loop.
- `reconcile_once` runs one pass under the caller's `Election`.

## Invariants

- Only the leader lease holder reconciles; an election error means not leader.
- A namespaced prune needs a controller owner reference with the resource's
  UID; a cluster-scoped one needs the expected labels and field manager, and
  deletes with a UID precondition.
- `project` keeps `lastTransitionTime` while a condition's status is unchanged.
- A `TerminationBudget` covers runtime deadline, SIGKILL reserve and preStop
  cost within a positive grace period; probe timing fields are all at least 1.
- Replica-layer totals are a multiple of the shard count; a shard split adds at
  most one shard, only above a strict threshold.
- A PVC is never shrunk, and grows only if its StorageClass allows expansion.
- A profile is checked against the `InstanceScope` before any Secret read or
  key generation; an `Issuer` never sees the private key.

## Published language

The `ManagedService` contract, render plans, capacity and resize planners,
lifecycle validation and certificate lifecycle. The public modules `service`,
`crd`, `resize`, `lease`, `stateful`, `render` (and submodules), `controller`,
`certificate` (including `certificate::profile`), `lifecycle`, `metrics` and
`llm` keep their paths and feature gates (`src/api/`), because each holds
names the root does not re-export. lumen glob-imports `service_k8s::lease::*`,
so that module must export exactly `Election` and `spawn`; lumen's release CI
runs `stateful_instance_render` and `stateful_adapter_equivalence` by name.

## Exceptions and debts

- **Checker exceptions (long-term):**
  - B2 (`schemars`): `ReplicaLayerPolicy`, `ShardSplitPolicy`, `Condition`,
    `ProbeTiming` and `LifecyclePolicy` derive `JsonSchema`, a compile-time
    description of the same serde wire shape. Downstream CRD specs and
    statuses embed them as-is (for example `Vec<service_k8s::Condition>` in
    lumen and tape), so their generated schemas must not change, and an
    interfaces copy would duplicate the wire contract. The derive does no I/O.
- **Tracked for P2:** public fields on `Election`, `InstanceScope`,
  `ReadyFacts`, `ReadinessTarget`, `PruneTarget`, `ClusterScopedChild`,
  `ReconcilePlan`, `RenderCtx`, the render `*Plan` types, `Condition`,
  `ClusterSpec`, `ResourceSpec`; bare id `IssuerId(pub String)`; `anyhow` in
  `reconcile_plan`, `run` and `parse_storage_bytes`.
