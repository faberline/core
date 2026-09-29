# surface

surface models a user interface as a renderer-neutral element tree, below any
renderer or framework runtime. Framework adapters produce the tree, renderers
paint it, and native readers, tests and parity comparators inspect its
serializable snapshot without a browser or a toolkit-private tree. In core,
ui-runtime renders components into it; downstream, jet's WASM runtime and its
TSX-to-Rust output build on it.

**Form:** shared kernel · **Depends on:** — · **Crate:** [`crates/surface`](../../crates/surface)

## Model

- **Element** — one node of the live tree: an intrinsic node (a static tag,
  `Props`, children), a text leaf, a `Component` invocation, `Empty` (a
  conditional null) or a `Fragment` (a transparent list).
- **Props** — the host props shared by runtimes and renderers: id, class name,
  style, value, input type, placeholder, checked, ARIA label, `html_for`,
  disabled, and three event callbacks.
- **Callback** — `Callback<P>`, a cloneable event handler typed by its payload:
  `()` for click, `String` for change, `bool` for checked change.
- **Component** — a named render function (`ComponentFn`) with type-erased
  props. A framework runtime expands it into an element tree.
- **surface snapshot** — `SurfaceSnapshot`: a schema-versioned, serializable,
  flat list of `SurfaceNode`s captured from an element tree.
- **SurfaceNode** — one snapshot node: its structural `node_id`, its
  `semantic_id`, its parent, its kind (`SurfaceNodeKind`: element, text or
  component), tag or component name, a11y role, accessible name, text,
  `SurfaceProps` and optional bounds.
- **semantic id** — the key comparators and tests align nodes by: the
  element's `props.id` when set, otherwise its structural path.
- **a11y role** — the accessibility role a snapshot derives from a tag, such as
  `button`, `checkbox`, `textbox`, `navigation` or `listitem`.
- **SurfaceProps** — the serializable projection of `Props`; each callback is
  reduced to a `has_on_*` flag.
- **SurfaceRect** — the layout bounds (`x`, `y`, `w`, `h`) a renderer writes
  back into a snapshot with `set_bounds`.

## Ports

None.

## Invariants

- A snapshot built by `SurfaceSnapshot::from_element` carries
  `SurfaceSnapshot::SCHEMA_VERSION` (currently 1).
- Node ids are structural paths: the root is `root`, a child is
  `<parent>/<index>`, and a fragment's children are `<node>/fragment/<index>`
  under the fragment's own parent. Fragments and `Empty` add no node.
- A snapshot holds no callbacks, only whether each one is present, so it can
  be serialized and compared.
- A `Component` element is recorded as one node named after the component; the
  snapshot does not expand it.
- The accessible name is the non-empty ARIA label; otherwise the trimmed text
  of a button, label or table cell, or the value, then the placeholder, of an
  input or textarea.
- Snapshot nodes have no bounds until a renderer sets them; `set_bounds`
  returns `false` for an unknown node id.
- The live tree is single-threaded: callbacks and component props are held in
  `Rc`.

## Published language

The whole public API. surface is the shared kernel: every context may use it
without declaring a dependency, and a domain layer may name it. It uses no
other context.

## Exceptions and debts

- **Checker exceptions (P1):**
  - B1 (naming): the crate is not split into the kernel layout yet. It is
    SPEC-MANAGED, so P1 leaves it untouched (ADR D18); P2 splits it after
    confirming that nothing regenerates it (D8).

  The single 423-line source file is a C1 size warning, not an exception; it
  stays visible in the report until the P2 split. serde on the snapshot types
  is allowed by policy (the snapshot is the wire format) and is not an
  exception either.
- **Tracked for P2:** public fields that jet builds with struct literals,
  including in the Rust it generates from TSX: `Props` and `Component`.
