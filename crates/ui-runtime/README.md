# ui-runtime

## Brief

This package now re-exports the standalone Surface runtime. It shares hook
storage and scheduling with direct users of that revision. See
[migration](../../docs/surface-migration.md) for the pin and access requirement.

`ui-runtime` is the renderer-neutral component runtime above `surface`: hooks,
fiber storage, mount, flush, and update scheduling over surface elements.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Renderer-Neutral Component Runtime | - | hooks, fiber storage, mount, flush, and scheduling |

### Renderer-Neutral Component Runtime

Renderers can run reusable component logic over surface elements without
coupling to a browser or native UI backend.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `ui_runtime`.
- Gate — behavior: `cargo test -p ui-runtime@0.4.14` - runtime hook and
  scheduling coverage
- Gate: `cargo test -p ui-runtime@0.4.14`
- Source: `crates/ui-runtime/src/lib.rs`; implementation in standalone Surface
- Evidence: `cargo test -p ui-runtime@0.4.14`; crates/ui-runtime/tests/
