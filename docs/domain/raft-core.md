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

- **NodeId, Term, Index** — newtypes over `u64`: build one with `new`, read
  the number with `get`; each serializes and prints as the bare number.
  `Term` is the Raft term (the election epoch); `next()` is the one step it
  takes. `Index` is 1-based; 0 means "before the first entry". It moves with
  `next`, `prev`, `saturating_add`, `saturating_sub` and `checked_add`, and
  the distance between two indices is a plain `u64` count.
- **Raft role** — `Role`: follower, candidate or leader.
- **Log entry** — `RaftEntry`: term, index, opaque command bytes and an
  `EntryKind` (`Command` or `Config`).
- **Hard state** — `PersistedState` (and the borrowed `PersistedStateRef`):
  term, vote and log, plus the commit index and the compaction point with its
  snapshot bytes, so a restarted node never votes twice in a term and can
  still serve lagging followers.
- **Raft membership** — `Membership`: voters and learners of one group,
  built with `Membership::new(voters, learners)` and read with `voters()`,
  `learners()` or `into_parts()`. `auto_membership(n)` takes node ids `0..n`, makes the largest odd prefix the
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

The node calls no port itself; these are the driver's two IO seams.

- `RaftStorage` — durable storage for one node's `PersistedState`: `load`,
  `save`, and `pin_committed_command`, whose `PinnedCommand` maps to a
  `CommandLease` holding one committed command's bytes. A driver saves
  `persisted_ref` through it before it sends messages or applies entries.
  Implemented by raft-runtime's `RaftStore`.
- `RaftDelivery` — sends one `RaftMsg` to a peer and returns its reply, plus
  `install_snapshot`. A driver drains `take_outgoing` into it and feeds the
  replies back with `handle`. Implemented by raft-runtime's HTTP peer client.

The unused `RaftTransport` trait was deleted in P2; `RaftDelivery` is the
port it was meant to be.

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
- **Debts:** none tracked. P2 made `Membership`'s fields private (ADR D2)
  and turned `NodeId`, `Term` and `Index` into newtypes; the downstream
  services that built `Membership` literals or used the ids as `u64` migrate
  when they move to the release that carries it.
