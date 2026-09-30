//! Link-time registry for CLI subcommands (feature `registry`).
//!
//! The one clap-typed piece of `cli-std`: each crate self-registers a
//! subcommand by implementing [`CliModule`] and adding it to [`CLI_MODULES`]
//! with `#[distributed_slice]`, so the main binary discovers command
//! definitions and dispatches them without a hand-maintained command table.

pub use crate::application::registry::{find_module, registered_names, CliModule, CLI_MODULES};
