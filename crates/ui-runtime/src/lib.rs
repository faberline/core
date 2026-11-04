//! Compatibility facade for the standalone Surface component runtime.
//!
//! The runtime owns React-like authoring semantics without depending on React
//! DOM, a browser, WASM, AppKit, or any concrete renderer. Components render
//! `surface::Element` trees; host adapters decide whether those trees are
//! painted by Jet WASM WebGPU, a native desktop backend, or a test recorder.
//!
//! This is the middle layer between the UI element model and renderer backends:
//!
//! ```text
//! Component/hooks -> Element tree -> layout/paint/backend
//! ```

// Re-exporting shares upstream type identity, hook storage and scheduling.
pub use ui_runtime_upstream::*;
