# Operations

How core is released, how downstream repos consume a release, how to check a
core change against them, and how to run the architecture checker.

The design of the shared end-to-end test tooling is in
[shared-test-tooling.md](shared-test-tooling.md).

Test coverage follows standard S8: the baseline is in [aw.toml](../../aw.toml), the job is `test: workspace, coverage` in .github/workflows/ci.yml, and `line` is raised in the same pull request that raises the measured value, never lowered.

The stability promise (semantic versioning, no breaking change before 2.0.0) and the stable surface of every crate are in [stability-promise.md](stability-promise.md).

The threat model of core (auth, TLS, audit, secrets, supply chain) is in [threat-model.md](threat-model.md).

## Releasing

core has one version for the whole workspace, in `[workspace.package].version`
of the root `Cargo.toml`. Every crate inherits it.

1. Bump `[workspace.package].version` and refresh `Cargo.lock` (for example
   with `cargo check --workspace`), so the lock file records the new version
   for every workspace crate.
2. Commit only those two files as `chore(release): vX.Y.Z`.
3. Tag that commit with a lightweight tag `vX.Y.Z` and push the tag.

`v0.4.14` (`029e765`) is an example: the release commit touches only
`Cargo.toml` and `Cargo.lock`. Tags are never moved or reused; a fix is a new
patch version.

A release that changes public API lists every change, and the downstream code
it breaks, in `docs/migration/`.

## How downstream repos consume core

Downstream repos depend on core crates by git tag, one entry per crate:

```toml
[dependencies]
service-k8s = { git = "https://github.com/faberline/core", tag = "v0.4.14" }
```

- All core crates in one downstream workspace should use the same tag. Two
  tags mean two copies of every shared type, and they do not unify.
- A repo that depends on `index-text` must carry the vendored `jieba-rs` patch
  in its own root manifest, because Cargo applies `[patch]` only from the root
  workspace (see the root [README](../../README.md#using-a-crate)).
- lumen's release-candidate workflow checks out the core commit pinned in its
  `Cargo.lock` and runs three core tests by name:
  `stateful_instance_render` and `stateful_adapter_equivalence` in
  service-k8s, and `adversarial_recovery` in raft-runtime. A test filter that
  matches nothing passes with zero tests, so renaming or moving one of these
  silently disables lumen's check. Tell lumen before doing either.

## Compatibility and pinning

This section is the rule for service repos. A service can follow it without asking.

### Which tag a service pins

- A service pins the newest core tag, or the newest patch tag of the previous minor version (one release behind). `v0.6.2` and `v0.5.9` are both allowed while `v0.6.2` is the newest tag. `v0.4.14` is not.
- One workspace pins one tag for every core crate (see above).
- A tag that is more than one minor version behind is unsupported: a fix lands on `main` and in a new tag, not in an old line.

### Moving to a new tag

1. Read the release notes page `docs/operations/release-notes-<version>.md` and the pages in `docs/migration/` for every version between the old and the new tag.
2. Change every core dependency to the new tag in one commit.
3. Run the service's own tests and its release-candidate checks.
4. If a breaking change in the notes touches the service, do its migration step in the same pull request.

### What a breaking change is

A change is breaking when it does any of these:

- Rust API: it removes, renames or moves a public item of any core crate; changes a public signature, field or enum variant; makes a public field private; or adds a required method to a public trait (a method without a default body).
- Wire and persisted formats: it changes any byte or field that a `wire_golden` test pins (`raft-core`, `raft-runtime`, `service-auth`, `service-backup` and `service-k8s` each have `tests/it/wire_golden.rs`), or makes a node fail to read data that an older tag wrote.
- Kubernetes: in `service-k8s`, it removes or renames a resource kind, a CRD field, a label, an annotation or an owner reference; adds a required CRD field; or changes the rendered output of an existing instance.

### What a breaking change needs

- A minor version bump (before 1.0 the minor version is the breaking one: `0.6.x` to `0.7.0`).
- A note under "Breaking changes" in the release notes page `docs/operations/release-notes-<version>.md`.
- A migration step in `docs/migration/` that names the code or resource to change.
- A golden test is never edited to make a break pass. If a golden file must change, the pull request has the three items above, and the review checks them.

### Renames of pinned tests

The three lumen release-candidate checks (`stateful_instance_render`, `stateful_adapter_equivalence`, `adversarial_recovery`; see above) stay by name. A rename or a move of one of them is a breaking change for lumen: tell lumen first, and list it in the release notes.

## Checking a change against downstream repos

A change that claims to keep the public API should be checked against the
repos that use it before it is tagged. Do this without touching their working
trees:

1. Clone each downstream repo locally (`git clone --local`) into a scratch
   directory.
2. In each clone, point every core git dependency at your core checkout with
   Cargo's `--config` patch override, one per crate:

   ```sh
   cargo check --workspace --all-targets \
     --config 'patch."https://github.com/faberline/core".raft-core.path="/path/to/core/crates/raft-core"' \
     --config '…one entry per core crate…'
   ```

3. Use one shared `CARGO_TARGET_DIR` for all clones to keep the build cache
   warm, and reset each clone's `Cargo.lock` before the run.
4. Run the same command once against the base commit (for example a
   `git archive` export of `main`) and once against the change. The change is
   compatible when every repo's result is the same in both runs.

`cargo-public-api` is the complementary check on the core side: a zero diff of
each crate's public API between the base and the change means no downstream
path moved.

## Architecture checker

The architecture contract is [`ddd.toml`](../../ddd.toml) at the repository
root; [architecture.md](../architecture.md) explains it. The checker lives in
the workspace repo:

```sh
uv run <workspace>/scripts/meta/rust_arch_contract.py check --repo core
```

`--repo` takes a repo name under `--root` (default `~/faberlines`) or a path.

| Flag | Effect |
|------|--------|
| `--json -` | Print the JSON report to stdout instead of the text summary. |
| `--json <path>` | Also write the JSON report to a file. |
| `--base <ref>` | Run the ratchet (R1) against the merge-base with `<ref>`: `ddd.toml` exceptions and policy may only shrink or tighten. |
| `--strict` | Treat the repo as enforced even while `[migration] enforce = false`. |
| `--baseline` | Report only; never fail. A baseline run is not evidence that the repo is clean. |
| `--top <n>` | Show the first `n` findings per rule in the text summary (default 5). |
| `--verbose` | Show every finding in the text summary. |

While `[migration] enforce = false`, findings are reported but do not fail the
run; `--strict` shows what an enforced run would do. Once `enforce = true`, any
error-level finding fails.

`scaffold --repo core` prints a draft `ddd.toml` without writing it. It is a
starting point for a new repo, not a way to update an existing contract.

When the checker reports a finding:

1. Fix it by placing the code in the right layer or context, if you can.
2. Otherwise add an `[[exceptions]]` entry with the rule, the subject the
   checker printed, the exact file paths (no globs, no directories), and a
   reason that says why the break is permanent or which change will remove
   it. A reason may not be empty or say "TODO".
3. Run with `--base main` (or the branch your change targets). Once the base
   has a `ddd.toml`, the ratchet (R1) fails on any new exception, any new path
   in an existing exception, any new or raised `[[large_files]]` entry, any
   `[policy]` change, and on turning `enforce` back to `false`. When a file
   named in an exception or `[[large_files]]` is split or moved, record the
   move in `[[renames]]` so the ratchet can follow it.

A checker bug, or a rule that does not fit core, is reported to the workspace
repo; core does not patch around it locally.
