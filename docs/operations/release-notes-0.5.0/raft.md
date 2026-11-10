# Release notes 0.5.0: raft-core and raft-runtime

Range checked: `v0.4.14` (`029e765e`) to `origin/main` (`b41c6a3f`).

## Crates checked

- `crates/raft-core`
- `crates/raft-runtime`

## Breaking changes

### raft-core: `NodeId`, `Term` and `Index` are newtypes, not `u64` aliases

Before: `pub type NodeId = u64;`, `pub type Term = u64;`, `pub type Index = u64;`.
After: `pub struct NodeId(u64);`, `pub struct Term(u64);`, `pub struct Index(u64);`, with `new(u64)` and `get(self) -> u64`. `Term` and `Index` also have `next`; `Index` also has `prev`, `saturating_sub`, `saturating_add` and `checked_add`.
Commits: `3f8ba418`, `be54de19`.

Migration: wrap every raw id with `NodeId::new(n)`, `Term::new(t)` or `Index::new(i)`, and read the number with `.get()`. Arithmetic on raw values must use the methods above.

### raft-core: `Membership` fields are private

Before: `pub voters: Vec<NodeId>`, `pub learners: Vec<NodeId>`.
After: `Membership::new(voters, learners)`, `voters(&self) -> &[NodeId]`, `learners(&self) -> &[NodeId]`, `into_parts(self) -> (Vec<NodeId>, Vec<NodeId>)`.
Commit: `55b56dab`.

Migration: replace struct literals and field reads with `Membership::new(...)` and the accessors.

### raft-core: `RaftTransport` trait removed

Before: `pub trait RaftTransport { fn deliver(&mut self, from: NodeId, out: Outgoing); }`.
After: `pub trait RaftDelivery: Send + Sync` with `fn deliver(...)`. The old trait is gone from the crate root.
Commit: `55da8a38`.

Migration: implement `RaftDelivery` instead. Its `Send + Sync` bound means the driver's transport must be thread-safe.

### raft-core: `RaftNode::learner_replication_gap` removed

Before: `pub fn learner_replication_gap(&self, peer: NodeId) -> Option<Index>`.
After: removed. Use `RaftNode::peer_match_index(&self) -> Option<Vec<(NodeId, Index)>>` (leader only) and compute the gap against the commit index.
Commit: `55da8a38`.

Migration: call `peer_match_index()` and subtract the peer's match index from the leader's commit index in the app.

### raft-runtime: `cluster` and `group` modules removed

Before: `pub mod cluster;` and `pub mod group;` (compatibility facades).
After: both modules are gone. Their items are re-exported at the crate root: `ClusterDims`, `ClusterTopology`, `peer_ordinal`, `GroupId`, `LEGACY_GROUP_ID`, `parse_peer_overrides`, `replica_mode`.
Commit: `b0024f59`.

Migration: change `raft_runtime::cluster::X` and `raft_runtime::group::X` to `raft_runtime::X`.

### raft-runtime: `ensure_static_membership_unchanged` removed

Before: `pub fn ensure_static_membership_unchanged(current: u32, desired: u32) -> Result<()>` in `cluster`.
After: removed.
Commit: `799d739e`.

Migration: delete the call. If the app needs the check, it must do it itself.

### raft-runtime: `RaftStore::cache_footprint` removed

Before: `pub fn cache_footprint(&self) -> usize`.
After: removed.
Commit: `799d739e`.

Migration: delete the call. Use the app's own memory metric if it needs one.

### raft-runtime: `GroupId` field is private

Before: `pub struct GroupId(pub String);`.
After: `pub struct GroupId(String);` with `GroupId::new(name: impl Into<String>)` and `as_str(&self) -> &str`.
Commit: `d4a4cc8e`.

Migration: replace `GroupId(s)` with `GroupId::new(s)` and `g.0` with `g.as_str()`.

### raft-runtime: `AssignmentEpoch` is a newtype, not a `u64` alias

Before: `pub type AssignmentEpoch = u64;`.
After: `pub struct AssignmentEpoch(u64);` with `AssignmentEpoch::new(epoch: u64)`, `get(self) -> u64` and `checked_next(self) -> Option<Self>`.
Commit: `f7339325`.

Migration: wrap raw epochs with `AssignmentEpoch::new(e)` and read them with `.get()`. Use `checked_next` where the app adds to an epoch.

### raft-runtime: `ClusterDims` fields are private

Before: `pub shard_count: u32`, `pub replicas_per_shard: u32`, `pub voter_count: u32`, `pub pod_name: String`.
After: accessors `shard_count() -> u32`, `replicas_per_shard() -> u32`, `voter_count() -> u32`, `pod_name() -> &str`. Construction: `ClusterDims::new(shard_count: u32, replicas_per_shard: u32, voter_count: u32, pod_name: impl Into<String>)`.
Commit: `2628a0de`.

Migration: replace field reads with the accessors and struct literals with `ClusterDims::new(shard_count, replicas_per_shard, voter_count, pod_name)`.

### raft-runtime: `HostConfig` fields are private

Before: `pub tick`, `pub pump`, `pub rpc_timeout`, `pub propose_timeout` (`Duration`) and `pub snapshot` (`SnapshotPolicy`).
After: accessors `tick()`, `pump()`, `rpc_timeout()`, `propose_timeout()` (each `-> Duration`) and `snapshot() -> SnapshotPolicy`. Builders `with_tick`, `with_pump`, `with_rpc_timeout`, `with_propose_timeout` and `with_snapshot`, from `HostConfig::default()`.
Commit: `71fcd0d9`.

Migration: read values through the accessors. Build with `HostConfig::default().with_tick(..).with_pump(..).with_rpc_timeout(..).with_propose_timeout(..).with_snapshot(..)` instead of a struct literal.

### raft-runtime: state-machine and membership port errors are typed

Before: the port methods returned `anyhow::Result<...>`.
After, by trait:
- `RaftStateMachine`: `admit_proposal`, `apply`, `snapshot`, `preflight_snapshot`, `snapshot_at`, `validate_snapshot` and `restore` return `Result<..., StateMachineError>`.
- `SnapshotPreparation::capture_at` returns `Result<..., StateMachineError>`.
- `PreparedSnapshot::write_to` returns `Result<(), StateMachineError>`.
- `MembershipPolicy::validate` returns `Result<(), MembershipError>`.

`StateMachineError` and `MembershipError` are exported at the crate root.
Commit: `81579b45`.

Migration: map the app's errors into `raft_runtime::StateMachineError` or `raft_runtime::MembershipError` at the port boundary.

## Fixes apps must take

### raft-runtime: `propose` no longer reports `Completed` for a superseded entry

Leader-path `propose` could report `Completed` for an entry that a newer entry at the same index had replaced. It now does not (`bd2b5455`, #102).
Migration: an app must treat only `Completed` as committed. Any other outcome means the caller must retry or check the state.

### raft-runtime: a truncated forwarded proposal is `Ambiguous`, not `Completed`

When a forwarded proposal's entry is truncated, `propose` acknowledged another proposal's `Completed`. The follower now gets `Ambiguous` (`bafa57be`, #103).
Migration: handle `Ambiguous` as "unknown outcome" and retry or check the state. Do not treat it as success.

## New

- raft-core: `RaftStorage` port (`domain/storage.rs`) and `CommandLease` / `PinnedCommand` types.
- raft-core: `RaftDelivery` port (replaces `RaftTransport`, see above).
- raft-core: `RaftNode::peer_match_index()`, the per-peer `match_index` of voters and learners on the leader (`8806fd92`).
- raft-core: `Membership::new`, `voters`, `learners` and `into_parts`.
- raft-core: constants `ELECTION_TIMEOUT_FLOOR_TICKS` and `HEARTBEAT_INTERVAL_TICKS` are re-exported at the crate root.
- raft-runtime: `RaftHost::status() -> RaftStatus` (`fc4d29f3`), with `PeerMatch` and `MembershipPhase`.
- raft-runtime: `ChunkHandler`, the boxed chunk callback type (`Box<dyn FnMut(&[u8]) -> std::io::Result<()> + Send>`) that `ChunkSink` uses by default.
- raft-runtime: `PeerMatch` carries per-peer `match_index` and the follower-observed leader commit, for replica lag (`8806fd92`).

## Not breaking

- Wire formats: the peer HTTP and RPC bodies are pinned byte for byte by golden tests (`crates/raft-core/tests/it/wire_golden.rs`, `crates/raft-runtime/src/tests/peer_wire_golden.rs`). The comment in the golden file says the bodies are literals captured before the change and "must stay byte-identical". Durable layout is pinned by `crates/raft-runtime/tests/it/durable_layout.rs`.
- Feature flags: `Cargo.toml` `[features]` are unchanged in both crates. The diff only removes the test declarations and adds `[lints] workspace = true`.
- File paths under `src/` moved to `domain/`, `application/`, `infrastructure/` and `interfaces/`. They are not public API unless re-exported at the crate root, and the root re-exports listed above are kept.
