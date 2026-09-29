//! Shared `build.rs` stamping: immutable or local git sha, built-at epoch, and target
//! triple as `cargo:rustc-env=<PREFIX>_*` directives.
//!
//! Consuming crates call [`stamp`] with their env-var prefix (e.g. `"LUMEN"`)
//! from their `build.rs`'s `fn main()`. An exact `<PREFIX>_SOURCE_REVISION`
//! carries a full Git SHA into archive builds. Without it, every stamp remains
//! best-effort: outside a git checkout the sha falls back to `"unknown"`, and
//! downstream `env!`/`option_env!` consumers degrade the same way.
//! Nothing here fails the build.

mod source_revision;
mod stamp;

pub use stamp::stamp;
