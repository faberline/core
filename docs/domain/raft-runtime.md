# raft-runtime

raft-runtime is the async host that runs a raft-core node for one replicated
service state machine. It owns the tick and pump loop, the h2c peer transport,
the single apply loop, snapshot and log compaction, read-your-write proposals,
ordered shutdown, multi-group registries and the fencing primitive for
committed work assignment. A service supplies a `RaftStateMachine`; the host
does the rest. No core crate depends on it; defer, keep, loom, lumen, relay,
sift and tape each run their replicated state through it.

**Form:** layered, with no domain layer: its domain is raft-core · **Depends on:** cli-std, peer-tls, raft-core, server-lifecycle, storage-durable, transport-h2c · **Crate:** [`crates/raft-runtime`](../../crates/raft-runtime)

## Model

- **Host** — `RaftHost`: one running Raft group. `HostConfig` sets the tick,
  pump, peer RPC and propose timeouts and the `SnapshotPolicy` (`Disabled`,
  `EveryEntries(n)` or `External`).
- **Proposal outcome** — `ProposalOutcome`: `Completed`,
  `RejectedBeforeAdmission`, `Ambiguous` or `DurabilityFailure`. A follower
  forwards a proposal to the leader's `/raft/publish`.
- **Proposal admission** — `AdmissionPermit`: memory the state machine admits
  for a proposal, held by the host from index allocation until apply.
- **Proposal backpressure** — `ProposalBackpressure`: a leader with no
  capacity refused the proposal before it allocated an index.
- **Learner admission** — `AdmissionRefused`: why a leader refused to add a
  learner.
- **Membership phase** — `MembershipPhase` (`Stable` or `Joint`), reported in
  `RaftStatus`. The status view's `RaftRole` adds learner to the Raft role.
- **Shutdown** — `shutdown_within` runs the `ShutdownPhase`s in order
  (quiesce, leadership handoff, background tasks, peer RPC drain) and returns
  a `HostShutdownReport` of `PhaseRecord`s.
- **Store** — `RaftStore`: file-backed persistence for one node under an
  `FsyncPolicy`. Inside it, a store generation pin keeps one log-store
  generation's files mapped while a command lease reads one committed
  command's bytes.
- **Group** — `GroupId` names a Raft group. `RaftRegistry` and
  `GroupRegistry` serve many groups behind one `/raft/*` router and route each
  peer RPC by group id.
- **Assignment** — `FencedAssignment` inside a state machine gives one owner a
  `FenceToken` (owner and assignment epoch) for one application-owned key.
- **Read consistency** — `ReadConsistency` (`Leader`, `Bounded(ms)`, `Any`),
  parsed from the `x-read-consistency` header.
- **Topology** — `ClusterDims` and `ClusterTopology` describe shards,
  replicas and voters. `ReplicaHostBuilder` builds the topology, peer
  transport, store and host together.

## Ports

- `RaftStateMachine` — the service's replicated state: apply, snapshot,
  restore, applied index, plus optional proposal admission and prepared
  snapshots (`SnapshotPreparation`, `PreparedSnapshot`). Implemented by defer,
  keep, loom, lumen, relay, sift and tape.
- `MembershipPolicy` — a product check applied to the `ClusterTopology` after
  the shared topology has been read and checked. Implemented by sift.

## Invariants

- The host is the only applier: each committed entry reaches the state
  machine once, in index order, from one worker. `apply` returns `Ok` only
  after the service's durable applied watermark reaches the index.
- Hard state is saved before the outbox is flushed, so no vote or append
  acknowledgement leaves the node before it is durable.
- `propose` returns only after the entry is applied (read-your-write).
- Fencing: epochs start at 1 and never repeat; a token exists only after the
  assignment commits; expiry is an explicit committed transition, and apply
  never reads a replica clock; a renewal never shortens the deadline; a stale
  epoch or wrong owner is rejected identically on every replica.
- A secure replica host requires HTTPS and mutual TLS and never falls back to
  clear text. A TLS reload affects new peer connections only.
- A missing or unknown read-consistency header means `Leader`.

## Published language

Services import from the crate root, which re-exports every public type plus
raft-core's `Membership`, `auto_membership`, ids and refusal enums. Two
public modules keep their paths because they hold names the root does not
re-export (`src/api/`): `conformance`, whose `DeterministicHost` is a
runtime-free, socket-free, clock-free host for conformance tests, and `llm`.
lumen uses both. P2 deleted the old modules `cluster` and `group`: every name
in them is at the crate root, so keep, lumen, relay and tape import
`replica_mode`, `ClusterTopology` and the other topology items from the root.

## Exceptions and debts

- **Checker exceptions (P1):** No B2: the crate has no domain layer.
  - B3 `application->infrastructure`: `RaftHost` holds the concrete
    `RaftStore` and `PeerTransport`, `propose` forwards a publish envelope to
    the leader, and `ReplicaHostBuilder` opens the store and builds the
    transport itself. P2 adds a raft log store port and a peer client port,
    implemented in infrastructure and injected at spawn.
  - B3 `infrastructure->application`: the outbound peer RPC client is an
    `impl` block on the host's shared state and decodes `RaftStatus`,
    `RaftStore` names its files by `GroupId`, and the environment reader builds
    `ClusterDims` and `ClusterTopology`. P2 moves those value types to a layer
    both sides may use, and the RPC client becomes an adapter with its own
    state.
  - B3 `interfaces->infrastructure`: the peer HTTP handlers and the group
    registry decode the same wire envelopes the RPC client sends, and
    `DeterministicHost::open` takes a `RaftStore`. P2 moves the envelopes to a
    wire module shared by both sides, and the conformance host takes the store
    port.
- **Tracked for P2:**
  - `anyhow` in both ports (ADR D4), implemented in seven downstream repos.
  - Public fields built with struct literals (ADR D2): `HostConfig` in defer,
    keep, lumen, relay and tape; `FenceToken` in defer and relay;
    `ClusterDims`, `PeerAddr` and `ClusterStateView` in lumen.
  - Bare ids: `GroupId(pub String)` and `AssignmentEpoch = u64`.
