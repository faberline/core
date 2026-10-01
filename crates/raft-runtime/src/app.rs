//! The composition root: wiring that may use every layer.
//!
//! The public constructors that pick infrastructure adapters live here: the
//! `RaftHost` constructors hand the host a `RaftStore` and the HTTP peer client
//! as its ports, the `DeterministicHost` constructors hand the conformance host
//! a `RaftStore`, `ReplicaHostBuilder` opens the store and builds the mTLS peer
//! transport, and the topology constructors read the process environment
//! through the infrastructure layer and hand the values to the application
//! layer, which validates them.

mod conformance;
mod host;
mod replica_host;
mod topology_env;

pub use replica_host::ReplicaHostRuntime;
