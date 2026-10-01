# ui-runtime

ui-runtime is the renderer-neutral component runtime on top of the surface
element tree: React-style hooks, per-component fiber storage, mounting, and
the flush and update-scheduling loop. Components render `surface::Element`
trees; a host adapter decides whether those trees are painted by jet's WASM
renderer, a native desktop backend or a test recorder. Its user is jet, whose
WASM runtime re-exports the whole crate (`pub use ui_runtime::*`) and whose
TSX-to-Rust transpiler emits calls to its hooks.

**Form:** domain only · **Depends on:** surface (kernel) · **Crate:** [`crates/ui-runtime`](../../crates/ui-runtime)

## Model

- **fiber** — the storage of one mounted component: its `FiberId`, its ordered
  hook slots, a hook cursor reset at the start of every render, and a dirty
  flag.
- **FiberId** — the identity of a fiber, allocated in increasing order by the
  runtime. Its `u64` is private: `FiberId::new` wraps a raw id and `get`
  returns it.
- **hook slot** — one positional entry in a fiber: state (used by `use_state`
  and `use_reducer`), memo (`use_memo`, `use_callback`), ref (`use_ref`) or
  effect-once (`use_effect_once`).
- **StateSetter** — the handle `use_state` returns; `set` stores a value and
  marks the owning fiber dirty.
- **DispatchHandle** — the handle `use_reducer` returns; `dispatch` runs the
  reducer on the current state and marks the fiber dirty.
- **RefHandle** — the handle `use_ref` returns: a mutable cell that survives
  re-renders.
- **MemoDepHash** — one dependency of a memo, hashed to `u64` with `hash_dep`.
- **MountHandle** — the result of `mount`: the root fiber, its `Component` and
  the last rendered tree; `flush` re-renders when the fiber is dirty.
- **update scheduler** — the callback a renderer registers with
  `set_update_scheduler`; every state write calls it so the renderer can
  coalesce dirty fibers into a frame.
- **debug summaries** — with the `debug` feature, `debug_snapshot_fibers` and
  `debug_snapshot_hooks` describe fibers and hook slots for tooling; both
  identify a fiber by its `FiberId`.

## Ports

None. The update scheduler is a registered callback, not a trait.

## Invariants

- Hooks are positional: each hook call takes the next slot, and a call that
  finds a slot of another kind or type panics as a rules-of-hooks violation.
- A hook called outside a render panics.
- Writing state through `StateSetter` or `DispatchHandle` marks the fiber
  dirty; the re-render happens on the next `MountHandle::flush`, which clears
  the flag and reports whether it re-rendered.
- Mutating a ref never triggers a re-render.
- A memo recomputes only when its dependency hashes differ from the last
  render.
- `use_effect_once` runs its effect exactly once per fiber.
- The runtime and the update scheduler live in thread-local state; the runtime
  is single-threaded by design.
- `debug_snapshot_hooks` returns an empty list for an unknown fiber instead of
  panicking.

## Published language

The whole public API; domain-only contexts have no application layer. The
code lives in private modules under `src/domain/`, and `lib.rs` re-exports
their public names. Because jet re-exports the crate with a glob, every public
name is part of jet's API, so every module stays private.

## Exceptions and debts

- **Checker exceptions:** none. P2 removed the SPEC-MANAGED markers (nothing
  regenerated the crate, D8), moved the code into one private module per
  concept under `src/domain/`, and deleted the B1 naming exception; the C1
  size warning went with the 693-line file.
- **Debts:** none tracked. P2 made the `FiberId` field private (W5), and the
  debug API now takes and returns `FiberId`; jet's debug bridge has to
  convert at its edge with `FiberId::new(raw)` and `id.get()`.
