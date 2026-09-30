//! The composition root: wiring that may use every layer.
//!
//! The application use cases take domain ports (the parser, the source
//! walker, ...); the functions here build the infrastructure adapters for
//! them and keep compass's zero-argument conveniences, such as
//! `compass::check_paths` and `compass::outline`, at their public paths.

pub(crate) mod analysis;
pub(crate) mod check;
pub(crate) mod daemon;
pub(crate) mod outline;
