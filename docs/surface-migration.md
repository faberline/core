# Standalone Surface compatibility

`faberline/surface` owns the renderer-neutral UI implementation. Core's
`surface` and `ui-runtime` packages are compatibility facades. They re-export
the upstream crates at `1754a2e96e487ea693be558041abdf21ffc63d27`.
There is one hook store, scheduler and set of element types in the Cargo graph.
The `debug` feature forwards to the upstream runtime.

Jet must pin both packages to that same revision. Direct standalone values
then pass through Core's old names without conversion. Keep both pins together
when upgrading. Core's CLI libraries remain independent of this migration.

Cargo now sees two package versions with each UI name. Select the workspace
facades explicitly:

```sh
cargo test --locked -p surface@0.4.14 -p ui-runtime@0.4.14 --all-features
cargo check --locked --target wasm32-unknown-unknown -p surface@0.4.14 -p ui-runtime@0.4.14 --all-features
```

The existing snapshot and builder tests still run through Core's public names.
The mixed-source compatibility test also checks type assignment, independent
mounts, a shared scheduler and the forwarded debug API. This does not satisfy
the separate MicroDOM/layout/rendering backlog issues.

Surface is currently private. An authenticated local Git fetch works. Public
Core/Jet builds and hosted CI need access to that dependency before this change
can land. Repository visibility has not been changed.

The archived EC inventories now name the qualified Cargo packages and actual
test paths. Their old July generation digests have not been regenerated:
the installed AW CLI no longer has the legacy EC compiler. Treat those digest
records as pending metadata, not fresh qualification evidence. The Rust tests,
including their previously ignored contract wrappers, run directly.

## Local verification

On 2026-11-04, the facade checks passed on macOS with Rust 1.96.1:
10 Rust cases (including two contract wrappers), Clippy with warnings denied,
native and WASM compilation, and the strict architecture check with zero errors
or warnings. The two UI jobs are included in the hosted CI matrix. Hosted
results and legacy inventory regeneration remain separate pending gates.
