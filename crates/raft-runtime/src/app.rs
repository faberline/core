//! The composition root: wiring that may use every layer.
//!
//! The public constructors that pick infrastructure adapters live here: the
//! `RaftHost` constructors connect the HTTP peer client and hand it to the host
//! as its ports, and the topology constructors read the process environment
//! through the infrastructure layer and hand the values to the application
//! layer, which validates them.

mod host;
mod topology_env;
