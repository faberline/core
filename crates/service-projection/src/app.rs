//! The composition root: wiring that may use every layer.
//!
//! The public `ProjectionRegistry::new` builds the file-backed state store
//! under the registry root and hands the application the state store port.

mod registry;
