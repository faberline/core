//! Activating renewed TLS material in a process that is already serving.
//!
//! #3112 R2/R4/R5/R6/R7. Projecting a renewed Secret does not make a running
//! process use it, and the naive fixes are both wrong: restarting every member
//! for each short-lived leaf spends the disruption budget of a three-voter group
//! on routine renewal, and swapping the config in place risks a window where the
//! new material is half-installed.
//!
//! So the shape here is build-then-swap. A candidate is read, fully validated
//! (see [`crate::material`]), and turned into finished rustls configs *before*
//! any lock is taken for writing; the swap itself is a pointer move. Every
//! failure path — unreadable file, malformed PEM, wrong key, wrong identity —
//! leaves the previously activated generation exactly as it was, still serving.
//!
//! ### Two things this deliberately does not do
//!
//! It does not touch the listener. New handshakes read the current config when
//! they are accepted, so activation is atomic per connection and connections
//! already accepted finish on the configuration they started with, inside the
//! ordinary drain window.
//!
//! It does not fail open. If the last known good material expires and nothing
//! valid has replaced it, the accessors return `None` and the caller refuses the
//! connection. An expired identity that keeps serving is worse than a refused
//! connection, because nothing downstream can tell it apart from a healthy one.

mod configs;
mod profile;
mod reloadable;
mod source;
mod status;
mod watcher;

pub use profile::TlsRuntimeProfile;
pub use reloadable::ReloadableTls;
pub use source::{FileMaterialSource, MaterialSource, MemoryMaterialSource};
pub use status::TlsReloadStatus;
pub use watcher::{spawn_material_watcher, DEFAULT_MATERIAL_POLL_INTERVAL};
