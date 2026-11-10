//! Compatibility facade for the pinned standalone Surface package.
//!
//! `surface` is deliberately below any renderer or framework runtime. It
//! owns the UI element tree shape that Jet WASM TSX/Vue/etc. adapters produce,
//! plus a serializable snapshot form that native desktop readers, renderers, tests,
//! and parity comparators can inspect without a browser or toolkit-private tree.

pub use surface_upstream::*;
