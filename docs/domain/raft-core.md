# raft-core

raft-core models Raft consensus as a tick-driven state machine that does no IO.
A `RaftNode` takes ticks, peer messages and proposals; it hands back the
messages to deliver, the hard state to persist and the committed entries to
apply. The caller supplies the network, the persistence and the replicated
state machine. In core, raft-runtime is the async host that drives it.
Downstream, loom, lumen, relay and sift depend on it directly; defer, keep and
tape use its types through raft-runtime's re-exports.

**Form:** domain only · **Depends on:** — · **Crate:** [`crates/raft-core`](../../crates/raft-core)

## Model

- **NodeId, Term, Index** — `u64` aliases. `Term` is the Raft term (the
  election epoch). `Index` is 1-based; 0 means "before the first entry".
- **Raft role** — `Role`: follower, candidate or leader.
- **Log entry** — `RaftEntry`: term, index, opaque command bytes and an
  `EntryKind` (`Command` or `Config`).
- **Hard state** — `PersistedState` (and the borrowed `PersistedStateRef`):
  term, vote and log, plus the commit index and the compaction point with its
  snapshot bytes, so a restarted node never votes twice in a term and can
  still serve lagging followers.
- **Raft membership** — `Membership`: voters and learners of one group.
  `auto_membership(n)` takes node ids `0..n`, makes the largest odd prefix the
  voters and a trailing even node a learner; `n = 0` counts as 1.
- **Configuration** — `ConfState`: the membership in force, the outgoing
  membership while a joint change is in flight, and the config generation, a
  counter bumped on every membership change. It has its own binary codec.
- **Messages** — `RaftMsg` wraps vote, append, install-snapshot and
  timeout-now requests and responses. `Outgoing` is a message the driver must
  deliver to one node.
- **Refusals** — `PromotionRefused`, `DemotionRefused`, `RemovalRefused` and
  `TransferRefused` say why a leader refused a membership or leadership change.
- **Timing** — `ELECTION_TIMEOUT_FLOOR_TICKS` (50) and
  `HEARTBEAT_INTERVAL_TICKS` (3). A node's election timeout is the floor plus
  its node id, in ticks.

## Ports

- `RaftTransport` — how a driver delivers outgoing messages. No core crate or
  downstream repo uses it; raft-runtime drains `take_outgoing` itself.

## Invariants

- The leader advances the commit index only to an entry of its current term
  that a majority of voters hold; during a joint change it needs a majority of
  both the incoming and the outgoing voters.
- `propose`, `propose_config` and `add_learner` return `None` unless the node
  is the leader and no leadership transfer is in flight.
- A membership change is a joint configuration with the next generation. It is
  refused while another change is in flight, when it targets the leader, when
  it would empty the voter set, or when it would lower fault tolerance.
- `adopt_conf` accepts only a strictly greater config generation.
  Configuration entries are adopted into force and withheld from the consumer
  by `take_committed`.
- `compact` acts only when `snapshot_index < up_to <= last_applied`.
- A stale append success never moves a follower's replication progress back.
  A snapshot install at the follower's own snapshot index is accepted only
  when its term and bytes are the same; one below it is superseded and
  acknowledged.
- `from_persisted` restarts the node as a follower, with its commit index
  clamped between the snapshot index and the last log index.

## Published language

The whole public API; domain-only contexts have no application layer.
raft-runtime re-exports `Membership`, `auto_membership`, `NodeId`, `Term`,
`Index` and the four refusal enums, and downstream services use them from
either crate.

## Exceptions and debts

- **Checker exceptions (P1):** None. The crate depends only on `serde`, and
  the timing constants are not an exception (ADR D15).
- **Tracked for P2:**
  - `Membership` public fields, built with struct literals by defer, keep,
    loom, lumen, relay, sift and tape (ADR D2).
  - Bare `u64` ids: `NodeId`, `Term` and `Index` are aliases, not newtypes.
  - The unused `RaftTransport` trait (ADR D7).
