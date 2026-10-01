# ui-runtime

ui-runtime is the renderer-neutral component runtime on top of the surface
element tree: React-style hooks, per-component fiber storage, mounting, and
the flush and update-scheduling loop. Components render `surface::Element`
trees; a host adapter decides whether those trees are painted by jet's WASM
renderer, a native desktop backend or a test recorder. Its user is jet, whose
WASM runtime re-exports the whole crate (`pub use ui_runtime::*`) and whose
TSX-to-Rust transpiler emits calls to its hooks.

**Form:** whole-src domain · **Depends on:** surface (kernel) · **Crate:** [`crates/ui-runtime`](../../crates/ui-runtime)

## Model

- **fiber** — the storage of one mounted component: its `FiberId`, its ordered
  hook slots, a hook cursor reset at the start of every render, and a dirty
  flag.
- **FiberId** — the identity of a fiber, allocated in increasing order by the
  runtime.
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
  `debug_snapshot_hooks` describe fibers and hook slots for tooling.

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

The whole public API; whole-src domain contexts have no application layer.
Because jet re-exports it with a glob, every public name is part of jet's API,
and any module P2 adds must be private with `pub use` re-exports.

## Exceptions and debts

- **Checker exceptions (P1):**
  - B1 (naming): the crate is not split into the domain layout yet. It is
    SPEC-MANAGED, so P1 leaves it untouched (ADR D18); P2 splits it after
    confirming that nothing regenerates it (D8).

  The single 693-line source file is a C1 size warning, not an exception; it
  stays visible in the report until the P2 split.
- **Tracked for P2:** bare ids. `FiberId` exposes its `u64`, and the debug API
  takes and returns fiber ids as bare `u64` (jet's debug bridge passes one).
