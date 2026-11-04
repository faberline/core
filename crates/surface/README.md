# surface

## Brief

This package now re-exports the standalone `faberline/surface` implementation.
See [migration](../../docs/surface-migration.md) for its immutable pin,
type identity, access requirement and validation commands.

`surface` is the renderer-neutral UI element model shared by Jet WASM, native
desktop readers, renderers, and parity tools.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Renderer-Neutral UI Surface Model | - | serializable element, props, callback, and snapshot model |

### Renderer-Neutral UI Surface Model

UI runtimes and renderers can exchange deterministic renderer-neutral surface
trees without depending on a specific frontend backend.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `surface`.
- Gate — behavior: `cargo test -p surface@0.4.14` - surface model and snapshot
  coverage
- Gate: `cargo test -p surface@0.4.14`
- Source: `crates/surface/src/`
- Evidence: `cargo test -p surface@0.4.14`; crates/surface/src/
