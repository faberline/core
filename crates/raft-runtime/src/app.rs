//! The composition root: wiring that may use every layer.
//!
//! The public constructors that read the process environment live here. They
//! read it through the infrastructure layer and hand the values to the
//! application layer, which validates them.

mod topology_env;
