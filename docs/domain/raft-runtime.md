# raft-runtime

raft-runtime is the async host that runs a raft-core node for one replicated
service state machine. It owns the tick and pump loop, the h2c peer transport,
the single apply loop, snapshot and log compaction, read-your-write proposals,
ordered shutdown, multi-group registries and the fencing primitive for
committed work assignment. A service supplies a `RaftStateMachine`; the host
does the rest. No core crate depends on it; defer, keep, loom, lumen, relay,
sift and tape each run their replicated state through it.

**Form:** layered; its consensus domain is raft-core, its own domain layer holds `GroupId` and the peer client port, and `src/app` is the composition root · **Depends on:** cli-std, peer-tls, raft-core, server-lifecycle, storage-durable, transport-h2c · **Crate:** [`crates/raft-runtime`](../../crates/raft-runtime)

## Model

- **Host** — `RaftHost`: one running Raft group. `HostConfig` sets the tick,
  pump, peer RPC and propose timeouts and the `SnapshotPolicy` (`Disabled`,
  `EveryEntries(n)` or `External`); start from `HostConfig::default()` and
  change fields with `with_tick`, `with_pump`, `with_rpc_timeout`,
  `with_propose_timeout` and `with_snapshot`.
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
  `FsyncPolicy`, and the implementation of raft-core's `RaftStorage` port. Inside it, a store generation pin keeps one log-store
  generation's files mapped while a command lease reads one committed
  command's bytes.
- **Group** — `GroupId` names a Raft group (`GroupId::new(name)`, read with
  `as_str()`). `RaftRegistry` and
  `GroupRegistry` serve many groups behind one `/raft/*` router and route each
  peer RPC by group id.
- **Assignment** — `FencedAssignment` inside a state machine gives one owner a
  `FenceToken` (owner and `AssignmentEpoch`) for one application-owned key.
  The epoch is a newtype over `u64` (`new`, `get`) that serializes and
  prints as the bare number.
- **Read consistency** — `ReadConsistency` (`Leader`, `Bounded(ms)`, `Any`),
  parsed from the `x-read-consistency` header.
- **Topology** — `ClusterDims` (`ClusterDims::new` or `from_env`, read
  through getters) and `ClusterTopology` describe shards, replicas and
  voters. `ReplicaHostBuilder`, whose build lives in the composition root,
  builds the topology, peer transport, store and host together.

## Ports

- `RaftStateMachine` — the service's replicated state: apply, snapshot,
  restore, applied index, plus optional proposal admission and prepared
  snapshots (`SnapshotPreparation`, `PreparedSnapshot`). Implemented by defer,
  keep, loom, lumen, relay, sift and tape. Every method returns
  `StateMachineError`.
- `MembershipPolicy` — a product check applied to the `ClusterTopology` after
  the shared topology has been read and checked. Implemented by sift. Returns
  `MembershipError`.
- raft-core's `RaftStorage` and `RaftDelivery` — the application host holds
  its store and its peer transport only through these. `RaftStore`
  (infrastructure) implements `RaftStorage`. The HTTP peer client
  (infrastructure) implements `RaftDelivery` and the crate-private domain
  port `PeerClient`, which carries status reads, proposal forwarding and the
  peer address book. The public `RaftHost::spawn*` constructors and
  `DeterministicHost::open` live in `src/app`: they take the caller's
  `RaftStore`, build the peer client, and hand both to the host as ports.

Both errors wrap an implementor's own error in `Other`, built with
`StateMachineError::other(e)` or with `?` from an `anyhow::Error`. The host
hands an `anyhow::Error` converted with `?` back unchanged, so a caller's
`downcast_ref` finds the implementor's type as before; a
`ProposalBackpressure` from `admit_proposal` reaches the caller of `propose`
whichever way it was wrapped.

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

- **Checker exceptions:** none. P2 removed the three P1 B3 exceptions:
  - `application->infrastructure`: the host reaches its store and peers
    through the ports above instead of holding `RaftStore` and
    `PeerTransport`, and `ReplicaHostBuilder`'s build moved to `src/app`.
  - `infrastructure->application`: the peer RPC client is an adapter with
    its own state, `GroupId` moved to the domain layer, and the environment
    readers for `ClusterDims` and `ClusterTopology` moved to `src/app`.
  - `interfaces->infrastructure`: the peer HTTP handlers and the group
    registry decode their own copy of the wire bodies
    (`interfaces/peer_http/wire.rs`; a test checks both copies encode the
    same bytes), and `DeterministicHost::open` moved to `src/app`.
- **Debts:** none tracked. P2 made the `HostConfig` and `ClusterDims` fields
  private (ADR D2), made `GroupId`'s field private and turned
  `AssignmentEpoch` into a newtype. `FenceToken`, `ActiveAssignment`,
  `PeerAddr`, `ClusterStateView` and `RaftStatus` keep public fields on
  purpose: they are wire or persisted shapes.
