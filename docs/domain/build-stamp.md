# build-stamp

build-stamp is the shared `build.rs` helper that stamps version metadata into a
binary at compile time. A build script calls `stamp(prefix)`; it prints Cargo
`cargo:rustc-env` directives for the git revision, the build time and the
target triple, so the binary can report them through `env!` or `option_env!`
without copying build-script logic. It is consumed as a `[build-dependencies]`
crate. No core crate depends on it; the service and CLI crates of downstream
repos such as lumen, sift, pgpool, relay, tape, defer, courier and mesh do.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/build-stamp`](../../crates/build-stamp)

## Model

- **prefix** — the caller's environment-variable prefix, for example `LUMEN`.
  Every variable below is named `<PREFIX>_*`.
- **git sha** — `<PREFIX>_GIT_SHA`: the source revision when one is supplied,
  else the 8-character short SHA of `HEAD`, else `unknown`.
- **source revision** — `<PREFIX>_SOURCE_REVISION`, read from the build
  environment: an exact full Git SHA that archive and image builds pass in
  because they have no git checkout.
- **built-at** — `<PREFIX>_BUILT_AT`: whole seconds since the Unix epoch, as a
  decimal string.
- **target** — `<PREFIX>_TARGET`: the target triple Cargo built for, taken from
  the build script's `TARGET` variable.
- **rerun hints** — `cargo:rerun-if-changed` on the workspace `.git/HEAD`, so
  the stamped SHA follows new commits, and `cargo:rerun-if-env-changed` on the
  source-revision variable.

## Ports

None.

## Invariants

- Stamping never fails the build. A missing git binary, a directory outside a
  git checkout, a missing `TARGET` or a clock before the epoch each yield the
  value `unknown`.
- A source revision is accepted only when it is exactly 40 ASCII hex
  characters; it is then lower-cased. Any other value is ignored and the
  short-SHA fallback applies.
- The short SHA is read with the `GIT_DIR`, `GIT_WORK_TREE` and related
  repository-locating variables removed, so git resolves the repository from
  the working directory rather than from an inherited environment.
- The `HEAD` rerun hint is printed only when `../../.git/HEAD` exists; in a
  linked worktree `.git` is a file, not a directory.
- The source-revision rerun hint is always printed, so changing the variable
  re-runs the build script.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. That API is the single function `build_stamp::stamp`.

## Exceptions and debts

- **Checker exceptions:** none. Running a process, reading the environment
  and reading the clock are the job of an infrastructure crate.
- **Debts:** none tracked.
